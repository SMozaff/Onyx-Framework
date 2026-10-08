use std::sync::{Arc, Mutex};

use tracing::Instrument;
use tracing::{span, Id, Subscriber};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<(String, bool)>>>);

impl<S> Layer<S> for Capture
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let has_parent = ctx.span(id).and_then(|span| span.parent()).is_some();
        self.0
            .lock()
            .unwrap()
            .push((attrs.metadata().name().to_string(), has_parent));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn command_span_is_child_of_http_request_span() {
    let capture = Capture::default();
    let records = capture.0.clone();
    let subscriber = tracing_subscriber::registry().with(capture);
    let _guard = tracing::subscriber::set_default(subscriber);
    let request = tracing::info_span!("http.request");
    async {
        let command = tracing::info_span!("command.execute");
        async {}.instrument(command).await;
    }
    .instrument(request)
    .await;
    let records = records.lock().unwrap();
    assert!(records
        .iter()
        .any(|(name, parent)| name == "http.request" && !parent));
    assert!(records
        .iter()
        .any(|(name, parent)| name == "command.execute" && *parent));
}
