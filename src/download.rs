use crate::{Config, Error, Event, WriteMethod};
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
use std::{
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs::OpenOptions;
use tokio_util::sync::CancellationToken;
use url::Url;

pub struct PreparedDownload {
    pub config: Config,
    pub headers: Arc<HeaderMap>,
    pub local_addr: Arc<[IpAddr]>,
    pub resp: Option<Arc<Mutex<Option<Response>>>>,
    pub on_event: Box<dyn FnMut(Event) + Send + Sync>,
}

/// 这个函数允许通过 drop Future 的方式来取消
pub async fn prefetch(
    url: Url,
    config: Config,
    mut on_event: impl FnMut(Event) + Send + Sync + 'static,
) -> Result<(UrlInfo, PreparedDownload), Error> {
    let headers: Arc<_> = config
        .headers
        .iter()
        .map(|(k, v)| (k.parse(), v.parse()))
        .filter_map(|(k, v)| k.ok().zip(v.ok()))
        .collect::<HeaderMap>()
        .into();
    let local_addr: Arc<[_]> = config.local_address.clone().into();
    let client = build_client(
        &headers,
        config.proxy.as_deref(),
        config.accept_invalid_certs,
        config.accept_invalid_hostnames,
        local_addr.first().copied(),
    )?;
    let mut retry_count = 0;
    let (info, resp) = loop {
        match client.prefetch(url.clone()).await {
            Ok(t) => break t,
            Err((e, t)) => {
                on_event(Event::PrefetchError(format!("{e:?}")));
                retry_count += 1;
                if retry_count >= config.retry_times {
                    return Err(Error::PrefetchTimeout(e));
                }
                tokio::time::sleep(t.unwrap_or(config.retry_gap)).await;
            }
        }
    };
    let prepared = PreparedDownload {
        config,
        headers,
        local_addr,
        resp: Some(Arc::new(Mutex::new(Some(resp)))),
        on_event: Box::new(on_event),
    };
    Ok((info, prepared))
}

impl PreparedDownload {
    /// 不能通过 drop Future 来终止这个函数，否则写入内容将会不完整
    pub async fn start(
        self,
        info: UrlInfo,
        save_path: PathBuf,
        cancel_token: CancellationToken,
    ) -> Result<(), Error> {
        let Self {
            config,
            headers,
            local_addr,
            resp,
            mut on_event,
        } = self;
        on_event(Event::Prefetch(info.clone()));
        let pusher = get_pusher(
            &info,
            config.write_method,
            config.write_buffer_size,
            &save_path,
        );
        let pusher = tokio::select! {
              () = cancel_token.cancelled() => {
                  on_event(Event::End { is_cancelled: true });
                  return Ok(());
              },
              pusher = pusher => pusher.map_err(Error::Io)?,
        };
        let puller = FastDownPuller::new(FastDownPullerOptions {
            url: info.final_url,
            headers,
            proxy: config.proxy.as_deref(),
            available_ips: local_addr,
            accept_invalid_certs: config.accept_invalid_certs,
            accept_invalid_hostnames: config.accept_invalid_hostnames,
            file_id: info.file_id,
            resp,
        })?;
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
                        config.downloaded_chunk.into_iter(),
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
        loop {
            tokio::select! {
                () = cancel_token.cancelled() => result.abort(),
                e = result.event_chain.recv() => match e {
                    Ok(e) => on_event((&e).into()),
                    Err(_) => break,
                }
            }
        }
        result.join().await?;
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
        let pusher = BoxPusher::new(
            MmapFilePusher::new(&save_path, info.size)
                .await
                .map_err(|e| format!("{e:?}"))?,
        );
        return Ok(pusher);
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(false)
        .open(&save_path)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let pusher = FilePusher::new(file, info.size, buffer_size)
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(BoxPusher::new(pusher))
}
