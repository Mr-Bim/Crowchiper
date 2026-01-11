#![allow(dead_code)]

use chromiumoxide::cdp::browser_protocol::network::ClearBrowserCookiesParams;
use chromiumoxide::cdp::browser_protocol::web_authn::{
    AddVirtualAuthenticatorParams, AuthenticatorId, AuthenticatorProtocol, AuthenticatorTransport,
    EnableParams, GetCredentialsParams, VirtualAuthenticatorOptions,
};
use chromiumoxide::{Browser, BrowserConfig, Page};
use crowchiper::{ServerConfig, db::Database};
use futures::StreamExt;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::OnceCell;
use url::Url;

/// Shared runtime for all tests in this binary
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Browser temp directory path (set once, used by both runtime and browser)
static BROWSER_DATA_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

fn browser_data_dir() -> &'static std::path::PathBuf {
    BROWSER_DATA_DIR
        .get_or_init(|| std::env::temp_dir().join(format!("chromiumoxide-{}", std::process::id())))
}

/// Cleanup function registered with libc::atexit.
/// Removes the browser's temporary user data directory at process exit.
/// We use atexit because Rust doesn't run destructors on statics.
extern "C" fn cleanup_browser_data() {
    if let Some(dir) = BROWSER_DATA_DIR.get() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub fn runtime() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        // Register cleanup to run at process exit.
        // Safety: cleanup_browser_data is a valid extern "C" fn with no arguments.
        unsafe {
            libc::atexit(cleanup_browser_data);
        }

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create runtime")
    })
}

/// Shared browser instance across all tests in this binary
static BROWSER: OnceCell<Browser> = OnceCell::const_new();

async fn get_browser() -> &'static Browser {
    BROWSER
        .get_or_init(|| async {
            let (browser, mut handler) = Browser::launch(
                BrowserConfig::builder()
                    .user_data_dir(browser_data_dir().clone())
                    .build()
                    .expect("Failed to build config"),
            )
            .await
            .expect("Failed to launch browser");

            // Spawn handler to process browser events
            tokio::spawn(async move {
                while let Some(event) = handler.next().await {
                    if event.is_err() {
                        break;
                    }
                }
            });

            browser
        })
        .await
}

pub struct TestContext {
    pub page: Page,
    pub base_url: String,
    pub db: Database,
    pub authenticator_id: AuthenticatorId,
    server_handle: tokio::task::JoinHandle<()>,
}

pub async fn setup() -> TestContext {
    TestSetup::new().build().await
}

pub async fn setup_with_prf() -> TestContext {
    TestSetup::new().with_prf(true).build().await
}

pub async fn setup_with_base(base: Option<&str>) -> TestContext {
    TestSetup::new().with_base(base).build().await
}

pub async fn setup_with_options(base: Option<&str>, enable_prf: bool) -> TestContext {
    TestSetup::new()
        .with_base(base)
        .with_prf(enable_prf)
        .build()
        .await
}

pub async fn setup_with_no_signup() -> TestContext {
    TestSetup::new().with_no_signup(true).build().await
}

/// Builder for test setup with various options
pub struct TestSetup<'a> {
    base: Option<&'a str>,
    enable_prf: bool,
    no_signup: bool,
    clear_cookies: bool,
}

impl<'a> TestSetup<'a> {
    pub fn new() -> Self {
        Self {
            base: None,
            enable_prf: false,
            no_signup: false,
            clear_cookies: false,
        }
    }

    pub fn with_base(mut self, base: Option<&'a str>) -> Self {
        self.base = base;
        self
    }

    pub fn with_prf(mut self, enable: bool) -> Self {
        self.enable_prf = enable;
        self
    }

    pub fn with_no_signup(mut self, no_signup: bool) -> Self {
        self.no_signup = no_signup;
        self
    }

    /// Clear auth cookies before the test starts.
    /// Navigates to the server first, clears cookies, then navigates to about:blank.
    pub fn with_clear_cookies(mut self) -> Self {
        self.clear_cookies = true;
        self
    }

    pub async fn build(self) -> TestContext {
        setup_with_full_options_internal(
            self.base,
            self.enable_prf,
            self.no_signup,
            self.clear_cookies,
        )
        .await
    }
}

async fn setup_with_full_options(
    base: Option<&str>,
    enable_prf: bool,
    no_signup: bool,
) -> TestContext {
    setup_with_full_options_internal(base, enable_prf, no_signup, false).await
}

async fn setup_with_full_options_internal(
    base: Option<&str>,
    enable_prf: bool,
    no_signup: bool,
    clear_cookies: bool,
) -> TestContext {
    // Start server on random port with in-memory database
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");

    // First bind to get the actual port, then create config with correct origin
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("Failed to get local address");
    let port = addr.port();

    // For WebAuthn, we use localhost as the RP ID and include the port in origin
    let rp_origin = Url::parse(&format!("http://localhost:{}", port)).expect("Invalid URL");
    let config = ServerConfig {
        base: base.map(|s| s.to_string()),
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin,
        jwt_secret: b"test-jwt-secret-for-testing".to_vec(),
        secure_cookies: false, // Tests run on localhost HTTP
        no_signup,
    };

    let app = crowchiper::create_app(&config);
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let base_url = format!(
        "http://localhost:{}{}{}",
        port,
        base.unwrap_or(""),
        env!("CONFIG_LOGIN_ASSETS")
    );

    // Get shared browser and create a new page
    let browser = get_browser().await;
    let page = browser
        .new_page("about:blank")
        .await
        .expect("Failed to create page");

    // Enable WebAuthn virtual authenticator for testing
    page.execute(EnableParams::builder().build())
        .await
        .expect("Failed to enable WebAuthn");

    let mut options_builder = VirtualAuthenticatorOptions::builder()
        .protocol(AuthenticatorProtocol::Ctap2)
        .transport(AuthenticatorTransport::Internal)
        .has_resident_key(true)
        .has_user_verification(true)
        .is_user_verified(true)
        .automatic_presence_simulation(true);

    if enable_prf {
        options_builder = options_builder.has_prf(true);
    }

    let options = options_builder
        .build()
        .expect("Failed to build authenticator options");

    let auth_response = page
        .execute(AddVirtualAuthenticatorParams::new(options))
        .await
        .expect("Failed to add virtual authenticator");

    // Clear cookies if requested - use CDP to clear all browser cookies
    if clear_cookies {
        page.execute(ClearBrowserCookiesParams::default())
            .await
            .expect("Failed to clear cookies via CDP");
    }

    TestContext {
        page,
        base_url,
        db,
        authenticator_id: auth_response.authenticator_id.clone(),
        server_handle,
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        self.server_handle.abort();
    }
}

/// Generate a random 32-byte test encryption key as base64url.
pub fn generate_test_key() -> String {
    use base64::Engine;
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut key);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key)
}

impl TestContext {
    pub async fn teardown(self) {
        // Page is dropped, server_handle is aborted in Drop
        // Browser is shared and stays open for other tests
    }

    /// Enable encryption for a user and inject the test key into the page.
    /// Returns the test key that was injected.
    ///
    /// This enables testing encrypted content without PRF support.
    /// Requires the app to be built with TEST_MODE=1.
    #[cfg(feature = "test-mode")]
    pub async fn enable_test_encryption(&self, user_id: i64) -> String {
        // Enable encryption in database (no PRF salt needed)
        self.db
            .encryption_settings()
            .enable_for_test(user_id)
            .await
            .expect("Failed to enable test encryption");

        // Generate and inject test key
        let test_key = generate_test_key();
        self.inject_test_key(&test_key).await;
        test_key
    }

    /// Inject a test encryption key into the page.
    /// The key should be base64url-encoded 32 bytes.
    pub async fn inject_test_key(&self, key: &str) {
        let script = format!("window.__TEST_ENCRYPTION_KEY__ = '{}';", key);
        self.page
            .evaluate(script)
            .await
            .expect("Failed to inject test key");
    }

    /// Clear all browser cookies via CDP
    pub async fn clear_cookies(&self) {
        self.page
            .execute(ClearBrowserCookiesParams::default())
            .await
            .expect("Failed to clear cookies via CDP");
    }

    /// Get credentials stored in the virtual authenticator
    pub async fn get_authenticator_credentials(&self) -> usize {
        let result = self
            .page
            .execute(GetCredentialsParams::new(self.authenticator_id.clone()))
            .await
            .expect("Failed to get credentials");
        result.credentials.len()
    }

    /// Create a new page in the shared browser with virtual authenticator
    pub async fn new_page(&self) -> Page {
        self.new_page_with_prf(false).await
    }

    /// Create a new page in the shared browser with PRF-enabled virtual authenticator
    pub async fn new_page_with_prf(&self, enable_prf: bool) -> Page {
        let page = get_browser()
            .await
            .new_page("about:blank")
            .await
            .expect("Failed to create page");

        // Enable WebAuthn virtual authenticator for the new page
        page.execute(EnableParams::builder().build())
            .await
            .expect("Failed to enable WebAuthn");

        let mut options_builder = VirtualAuthenticatorOptions::builder()
            .protocol(AuthenticatorProtocol::Ctap2)
            .transport(AuthenticatorTransport::Internal)
            .has_resident_key(true)
            .has_user_verification(true)
            .is_user_verified(true)
            .automatic_presence_simulation(true);

        if enable_prf {
            options_builder = options_builder.has_prf(true);
        }

        let options = options_builder
            .build()
            .expect("Failed to build authenticator options");

        page.execute(AddVirtualAuthenticatorParams::new(options))
            .await
            .expect("Failed to add virtual authenticator");

        page
    }

    pub async fn goto(&self, path: &str) {
        self.page
            .goto(&format!("{}{}", self.base_url, path))
            .await
            .expect("Failed to navigate");
    }

    pub async fn eval<T: serde::de::DeserializeOwned>(&self, expr: &str) -> T {
        self.page
            .evaluate(expr)
            .await
            .expect("Failed to evaluate")
            .into_value()
            .expect("Failed to deserialize")
    }

    /// Wait for a JavaScript condition to become true.
    /// Returns Ok(()) if condition is met, Err with message if timeout.
    /// Handles navigation by catching context errors and retrying.
    pub async fn wait_for(&self, condition: &str, timeout_ms: u64) -> Result<(), String> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            match self.page.evaluate(condition).await {
                Ok(value) => {
                    if let Ok(true) = value.into_value::<bool>() {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Context may have changed due to navigation, retry
                }
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(format!(
            "Condition `{}` not met within {}ms",
            condition, timeout_ms
        ))
    }
}
