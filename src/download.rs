use crate::{Config, Err2Str, Event, WriteMethod};
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
use reqwest::{Response, header::HeaderMap};
use std::{net::IpAddr, path::Path, sync::Arc};
use tokio::fs::OpenOptions;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Debug, Clone)]
pub struct PreparedDownload {
    pub config: Config,
    pub headers: Arc<HeaderMap>,
    pub local_addr: Arc<[IpAddr]>,
    pub resp: Arc<Mutex<Option<Response>>>,
}

/// 这个函数允许通过 drop Future 的方式来取消
pub async fn prefetch(
    url: Url,
    config: Config,
    mut on_event: impl FnMut(Event) + Send + Sync,
) -> Result<(UrlInfo, PreparedDownload), String> {
    let headers = Arc::new(HeaderMap::from_iter(
        config
            .headers
            .iter()
            .map(|(k, v)| (k.parse(), v.parse()))
            .filter_map(|(k, v)| k.ok().zip(v.ok())),
    ));
    let local_addr: Arc<[_]> = config.local_address.clone().into();
    let client = build_client(
        &headers,
        config.proxy.as_deref(),
        config.accept_invalid_certs,
        config.accept_invalid_hostnames,
        local_addr.first().cloned(),
    )
    .err2str()?;
    let (info, resp) = loop {
        match client.prefetch(url.clone()).await {
            Ok(t) => break t,
            Err((e, t)) => {
                on_event(Event::PrefetchError(format!("{e:?}")));
                tokio::time::sleep(t.unwrap_or(config.retry_gap)).await;
            }
        }
    };
    let prepared = PreparedDownload {
        config,
        headers,
        local_addr,
        resp: Arc::new(Mutex::new(Some(resp))),
    };
    Ok((info, prepared))
}

impl PreparedDownload {
    /// 不能通过 drop Future 来终止这个函数，否则写入内容将会不完整
    pub async fn start(
        self,
        info: UrlInfo,
        cancel_token: CancellationToken,
        mut on_event: impl FnMut(Event) + Send + Sync,
    ) -> Result<(), String> {
        let PreparedDownload {
            config,
            headers,
            local_addr,
            resp,
        } = self;
        let save_path = config.save_dir.join(info.filename());
        let pusher = get_pusher(
            &info,
            config.write_method,
            config.write_buffer_size,
            &save_path,
        );
        let pusher = tokio::select! {
              _ = cancel_token.cancelled() => {
                  on_event(Event::End { is_cancelled: true });
                  return Ok(());
              },
              pusher = pusher => pusher?,
        };
        let puller = FastDownPuller::new(FastDownPullerOptions {
            url: info.final_url,
            headers,
            proxy: config.proxy.as_deref(),
            available_ips: local_addr,
            accept_invalid_certs: config.accept_invalid_certs,
            accept_invalid_hostnames: config.accept_invalid_hostnames,
            file_id: info.file_id,
            resp: Some(resp),
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
        let cancel_monitor = tokio::spawn({
            let result = result.clone();
            let token = cancel_token.clone();
            async move {
                token.cancelled().await;
                result.abort();
            }
        });
        while let Ok(e) = result.event_chain.recv().await {
            on_event((&e).into());
        }
        result.join().await.err2str()?;
        cancel_monitor.abort();
        on_event(Event::End {
            is_cancelled: cancel_token.is_cancelled(),
        });
        Ok(())
    }
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
