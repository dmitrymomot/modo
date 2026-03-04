use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, ItemFn, Result};

pub fn expand(_attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let func: ItemFn = parse2(item)?;
    let func_body = &func.block;

    Ok(quote! {
        fn main() {
            rskit::tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime")
                .block_on(async {
                    // Initialize tracing
                    rskit::tracing_subscriber::fmt()
                        .with_env_filter(
                            rskit::tracing_subscriber::EnvFilter::try_from_default_env()
                                .unwrap_or_else(|_| rskit::tracing_subscriber::EnvFilter::new("info"))
                        )
                        .init();

                    // Load config
                    let config = rskit::config::AppConfig::from_env();

                    // Build app — user's code gets `app` binding
                    let app = rskit::app::AppBuilder::new(config);

                    let __rskit_result: std::result::Result<(), Box<dyn std::error::Error>> = {
                        let app = app;
                        async move #func_body
                    }.await;

                    if let Err(e) = __rskit_result {
                        rskit::tracing::error!("Application error: {e}");
                        std::process::exit(1);
                    }
                });
        }
    })
}
