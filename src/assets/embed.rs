use rust_embed::Embed;

/// Login assets (public, no auth required)
#[derive(Embed)]
#[folder = "dist/login/"]
pub struct LoginAssets;

/// App assets (protected, JWT required)
#[derive(Embed)]
#[folder = "dist/app/"]
pub struct AppAssets;

/// Dashboard assets (protected, admin only)
#[derive(Embed)]
#[folder = "dist/dashboard/"]
pub struct DashboardAssets;
