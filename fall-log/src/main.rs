use fall_log::*;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let _ = FallLog::new("fall-log".to_string(), std::io::stdout()).init();
    let span = span!(Level::INFO, "hello", trace_id = 1, span_id = 1);
    let _enter = span.enter();
    let mut fs = vec![];
    for i in 1..10 {
        fs.push(run_log(i));
    }
    for f in fs {
        f.await;
    }
    Ok(())
}

async fn run_log(i: u16) {
    info!("你好: {}", i);
}
