use crate::{Config, Err2Str, Event, Proxy, WriteMethod};
use fast_down::{
    BoxPusher, UrlInfo,
    file::FilePusher,
    http::Prefetch,
    invert,
    multi::{self, download_multi},
    single::{self, download_single},
    utils::{FastDownPuller, FastDownPullerOptions, build_client},
};
use parking_lot::Mutex;
use reqwest::header::HeaderMap;
use std::{path::Path, sync::Arc};
use tokio::fs::OpenOptions;
use tokio_util::sync::CancellationToken;
use url::Url;

pub async fn download(
    url: Url,
    config: Config,
    cancel_token: CancellationToken,
    on_info: impl FnOnce(&mut UrlInfo),
    mut on_event: impl FnMut(Event) + Send + Sync,
) -> Result<(), String> {
    let result = async {
        let headers = Arc::new(HeaderMap::from_iter(
            config
                .headers
                .iter()
                .map(|(k, v)| (k.parse(), v.parse()))
                .filter_map(|(k, v)| k.ok().zip(v.ok())),
        ));
        let proxy = match &config.proxy {
            Proxy::No => Some(""),
            Proxy::System => None,
            Proxy::Custom(proxy) => Some(proxy.as_str()),
        };
        let local_addr: Arc<[_]> = config.local_address.into();
        let client = build_client(
            &headers,
            proxy,
            config.accept_invalid_certs,
            config.accept_invalid_hostnames,
            local_addr.first().cloned(),
        )
        .err2str()?;
        let (mut info, resp) = loop {
            match client.prefetch(url.clone()).await {
                Ok(t) => break t,
                Err((e, t)) => {
                    on_event(Event::PrefectError(format!("{e:?}")));
                    tokio::time::sleep(t.unwrap_or(config.retry_gap)).await;
                }
            }
        };
        on_info(&mut info);
        let save_path = config.save_dir.join(info.filename());
        let pusher = get_pusher(
            &info,
            config.write_method,
            config.write_buffer_size,
            &save_path,
        )
        .await?;
        let puller = FastDownPuller::new(FastDownPullerOptions {
            url: info.final_url,
            headers,
            proxy,
            available_ips: local_addr,
            accept_invalid_certs: config.accept_invalid_certs,
            accept_invalid_hostnames: config.accept_invalid_hostnames,
            file_id: info.file_id,
            resp: Some(Arc::new(Mutex::new(Some(resp)))),
        })
        .err2str()?;
        let threads = if info.fast_download {
            config.threads.max(1)
        } else {
            1
        };
        let result = if info.fast_download {
            download_multi(
                puller,
                pusher,
                multi::DownloadOptions {
                    download_chunks: invert(
                        config.have_been_downloaded_chunk.into_iter(),
                        info.size,
                        config.chunk_window,
                    ),
                    retry_gap: config.retry_gap,
                    concurrent: threads,
                    pull_timeout: config.pull_timeout,
                    push_queue_cap: config.write_queue_cap,
                    min_chunk_size: config.min_chunk_size,
                    max_speculative: config.max_speculative,
                },
            )
        } else {
            download_single(
                puller,
                pusher,
                single::DownloadOptions {
                    retry_gap: config.retry_gap,
                    push_queue_cap: config.write_queue_cap,
                },
            )
        };
        Ok::<_, String>(result)
    };
    let result = tokio::select! {
        _ = cancel_token.cancelled() => {
            on_event(Event::End { is_cancelled: true });
            return Ok(());
        },
        res = result => res?,
    };
    let cancel_handle = tokio::spawn({
        let result = result.clone();
        let cancel_token = cancel_token.clone();
        async move {
            cancel_token.cancelled().await;
            result.abort();
        }
    });
    while let Ok(e) = result.event_chain.recv().await {
        on_event((&e).into());
    }
    result.join().await.err2str()?;
    cancel_handle.abort();
    on_event(Event::End {
        is_cancelled: cancel_token.is_cancelled(),
    });
    Ok(())
}

pub async fn get_pusher(
    info: &UrlInfo,
    write_method: WriteMethod,
    buffer_size: usize,
    save_path: &Path,
) -> Result<BoxPusher, String> {
    #[cfg(target_pointer_width = "64")]
    if info.fast_download && write_method == WriteMethod::Mmap {
        use fast_down::file::MmapFilePusher;
        let pusher = BoxPusher::new(MmapFilePusher::new(&save_path, info.size).await.err2str()?);
        return Ok(pusher);
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(false)
        .open(&save_path)
        .await
        .err2str()?;
    let pusher = FilePusher::new(file, info.size, buffer_size)
        .await
        .err2str()?;
    Ok(BoxPusher::new(pusher))
}
