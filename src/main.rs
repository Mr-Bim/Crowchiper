use std::net::SocketAddr;

use clap::Parser;
use crowchiper::cli::{
    Args, build_config, handle_create_admin, init_logging, load_jwt_secret, open_database,
    validate_rp_origin,
};
use crowchiper::create_app;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    init_logging(&args.log_format);

    let Some(jwt_secret) = load_jwt_secret(args.jwt_secret_file.as_deref()) else {
        std::process::exit(1);
    };

    let Some(db) = open_database(&args.database).await else {
        std::process::exit(1);
    };

    if args.create_admin {
        handle_create_admin(&db, &args.rp_origin, args.base.as_deref()).await;
    }

    let Some(rp_origin) = validate_rp_origin(&args.rp_origin) else {
        std::process::exit(1);
    };

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            error!(address = %addr, error = %e, "Failed to bind");
            std::process::exit(1);
        });

    let local_addr = listener.local_addr().unwrap();

    // In test mode, update rp_origin to include actual port when using port 0 with localhost
    #[cfg(feature = "test-mode")]
    let rp_origin = test_mode::maybe_update_rp_origin(rp_origin, args.port, local_addr.port());

    let config = build_config(
        args.base,
        db,
        args.rp_id,
        rp_origin,
        jwt_secret,
        args.no_signup,
    );
    let app = create_app(&config);

    info!(address = %local_addr, "Listening");

    #[cfg(feature = "test-mode")]
    println!("CROWCHIPER_READY port={}", local_addr.port());

    let make_service = app.into_make_service_with_connect_info::<SocketAddr>();
    if let Err(e) = axum::serve(listener, make_service).await {
        error!(error = %e, "Server error");
        std::process::exit(1);
    }
}

#[cfg(feature = "test-mode")]
mod test_mode {
    use url::Url;

    /// Update rp_origin to include the actual port when using port 0 with localhost.
    pub fn maybe_update_rp_origin(
        mut rp_origin: Url,
        requested_port: u16,
        actual_port: u16,
    ) -> Url {
        if requested_port == 0
            && rp_origin.host_str() == Some("localhost")
            && rp_origin.port().is_none()
        {
            rp_origin.set_port(Some(actual_port)).ok();
        }
        rp_origin
    }
}
