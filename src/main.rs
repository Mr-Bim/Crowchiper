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

    let config = build_config(
        args.base,
        db,
        args.rp_id,
        rp_origin,
        jwt_secret,
        args.no_signup,
    );
    let app = create_app(&config);

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            error!(address = %addr, error = %e, "Failed to bind");
            std::process::exit(1);
        });

    info!(address = %addr, "Listening");

    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "Server error");
        std::process::exit(1);
    }
}
