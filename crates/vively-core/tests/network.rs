use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Value, json};
use vively_core::{
    AudioService, Error, Result, VkClient,
    api::Page,
    auth::{AuthService, LoginContinuation, QrAuthStatus, QrValidationResult},
    client::{CaptchaResolver, Parameters, TokenRefresher, params},
    config::{CATALOG_USER_AGENT, VkConfig},
    error::CaptchaChallenge,
    library::MusicLibrary,
    lyrics::{GeniusService, LyricsContent, LyricsProvider, LyricsSource, MusixmatchService},
    models::{Audio, Playlist, SavedPlaylistCover},
    session::{Session, SessionPersistence, SessionStore, SessionTokenRefresher},
};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{body_string_contains, header, method, path},
};

fn client(server: &MockServer) -> VkClient {
    let mut config = VkConfig::android("test-client-secret").unwrap();
    config.api_host = server.uri().parse().unwrap();
    VkClient::new(config, "test-device").unwrap()
}

fn form(request: &Request) -> BTreeMap<String, String> {
    url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect()
}

fn audio() -> Audio {
    serde_json::from_value(json!({"id": 1, "owner_id": 7, "title": "Track", "artist": "Artist", "duration": 125, "is_explicit": false})).unwrap()
}

struct FixedRefresher(AtomicUsize);
#[async_trait]
impl TokenRefresher for FixedRefresher {
    async fn refreshed_token(&self, expired: &str) -> Result<Option<String>> {
        assert_eq!(expired, "expired");
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Some("fresh".into()))
    }
}

struct FixedCaptcha {
    calls: AtomicUsize,
    answer: Option<String>,
}
#[async_trait]
impl CaptchaResolver for FixedCaptcha {
    async fn resolve(&self, challenge: &CaptchaChallenge) -> Result<Option<String>> {
        assert_eq!(challenge.sid, "captcha-sid");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.answer.clone())
    }
}

fn captcha_response() -> Value {
    json!({"error": {"error_code": 14, "error_msg": "Captcha", "captcha_sid": "captcha-sid",
        "redirect_uri": "https://captcha.invalid/", "captcha_ts": 123.25, "captcha_attempt": 2,
        "is_sound_captcha_available": false}})
}

#[tokio::test]
async fn requests_encode_forms_and_decode_zstd() {
    let server = MockServer::start().await;
    let compressed = zstd::encode_all(br#"{"response":{"ok":true}}"#.as_slice(), 1).unwrap();
    Mock::given(method("POST"))
        .and(path("/method/test.call"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(header("x-vk-android-client", "new"))
        .respond_with(move |request: &Request| {
            let fields = form(request);
            assert_eq!(fields["q"], "a +&=/@,Ж");
            assert_eq!(fields["device_id"], "test-device");
            assert_eq!(fields["v"], "5.272");
            assert_eq!(fields["access_token"], "test-token");
            assert_eq!(fields["client_secret"], "test-client-secret");
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "zstd")
                .set_body_bytes(compressed.clone())
        })
        .expect(1)
        .mount(&server)
        .await;
    let response: Value = client(&server)
        .method(
            "test.call",
            params([("q", "a +&=/@,Ж".into()), ("v", "5.272".into())]),
            Some("test-token"),
        )
        .await
        .unwrap();
    assert_eq!(response, json!({"ok": true}));
}

#[tokio::test]
async fn refresh_and_captcha_retries_keep_headers_and_request_parameters() {
    let server = MockServer::start().await;
    let sequence = Arc::new(AtomicUsize::new(0));
    let seen = sequence.clone();
    Mock::given(method("POST"))
        .and(path("/method/test.call"))
        .and(header("user-agent", CATALOG_USER_AGENT))
        .respond_with(move |request: &Request| {
            let fields = form(request);
            assert_eq!(fields["q"], "query");
            match seen.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    assert_eq!(fields["access_token"], "expired");
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"error":{"error_code":5,"error_msg":"Expired"}}))
                }
                1 => {
                    assert_eq!(fields["access_token"], "fresh");
                    ResponseTemplate::new(200).set_body_json(captcha_response())
                }
                2 => {
                    assert_eq!(fields["access_token"], "fresh");
                    assert_eq!(fields["captcha_sid"], "captcha-sid");
                    assert_eq!(fields["success_token"], "success");
                    assert_eq!(fields["captcha_ts"], "123.250000");
                    assert_eq!(fields["captcha_attempt"], "2");
                    assert_eq!(fields["is_sound_captcha"], "0");
                    ResponseTemplate::new(200).set_body_json(json!({"response": 1}))
                }
                _ => panic!(),
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    let client = client(&server);
    let refresher = Arc::new(FixedRefresher(AtomicUsize::new(0)));
    let captcha = Arc::new(FixedCaptcha {
        calls: AtomicUsize::new(0),
        answer: Some("success".into()),
    });
    client.set_token_refresher(Some(refresher.clone())).await;
    client.set_captcha_resolver(Some(captcha.clone())).await;
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static(CATALOG_USER_AGENT));
    let result: i64 = client
        .method_with_headers(
            "test.call",
            params([("q", "query".into())]),
            headers,
            Some("expired"),
        )
        .await
        .unwrap();
    assert_eq!(result, 1);
    assert_eq!(refresher.0.load(Ordering::SeqCst), 1);
    assert_eq!(captcha.calls.load(Ordering::SeqCst), 1);
    assert_eq!(sequence.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn captcha_retries_stop_after_three_answers() {
    let server = MockServer::start().await;
    Mock::given(path("/method/test.call"))
        .respond_with(ResponseTemplate::new(200).set_body_json(captcha_response()))
        .expect(4)
        .mount(&server)
        .await;
    let client = client(&server);
    let resolver = Arc::new(FixedCaptcha {
        calls: AtomicUsize::new(0),
        answer: Some("success".into()),
    });
    client.set_captcha_resolver(Some(resolver.clone())).await;
    let result: Result<i64> = client.method("test.call", Parameters::new(), None).await;
    assert!(matches!(result, Err(Error::Captcha(_))));
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn cancelled_captcha_is_returned_without_retry() {
    let server = MockServer::start().await;
    Mock::given(path("/method/test.call"))
        .respond_with(ResponseTemplate::new(200).set_body_json(captcha_response()))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);
    client
        .set_captcha_resolver(Some(Arc::new(FixedCaptcha {
            calls: AtomicUsize::new(0),
            answer: None,
        })))
        .await;
    let result: Result<i64> = client.method("test.call", Parameters::new(), None).await;
    assert!(matches!(result, Err(Error::Captcha(_))));
}

#[tokio::test]
async fn refresh_is_tried_once_per_request() {
    let server = MockServer::start().await;
    Mock::given(path("/method/test.call"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"error":{"error_code":1117,"error_msg":"Expired"}})),
        )
        .expect(2)
        .mount(&server)
        .await;
    let client = client(&server);
    let refresher = Arc::new(FixedRefresher(AtomicUsize::new(0)));
    client.set_token_refresher(Some(refresher.clone())).await;
    let result: Result<i64> = client
        .method("test.call", Parameters::new(), Some("expired"))
        .await;
    assert!(matches!(result, Err(Error::Api(_))));
    assert_eq!(refresher.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn http_failures_keep_vk_errors_and_reject_non_vk_bodies() {
    let server = MockServer::start().await;
    for (name, body) in [("html", "<html>Error</html>"), ("json", "{}")] {
        Mock::given(path(format!("/method/{name}")))
            .respond_with(ResponseTemplate::new(503).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;
        let result: Result<Value> = client(&server).method(name, Parameters::new(), None).await;
        assert!(matches!(result, Err(Error::Http(503))));
    }
    Mock::given(path("/method/denied"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"error":{"error_code":7,"error_msg":"Denied"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let result: Result<Value> = client(&server)
        .method("denied", Parameters::new(), None)
        .await;
    assert!(matches!(result, Err(Error::Api(e)) if e.code == 7));
}

#[tokio::test]
async fn authentication_keeps_cookies_and_uses_confirmed_sid() {
    let server = MockServer::start().await;
    Mock::given(path("/oauth/get_anonym_token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "login_cookie=test; Path=/")
                .set_body_json(json!({"token":"anonymous"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.validateAccount"))
        .and(header("cookie", "login_cookie=test"))
        .and(body_string_contains("access_token=anonymous"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"response":{"sid":"validated","next_step":{"verification_method":"codegen"}}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/ecosystem.checkOtp")).and(body_string_contains("sid=validated")).and(body_string_contains("code=123456"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{"sid":"confirmed","can_skip_password":false,"profile":{"first_name":"Test"}}}))).expect(1).mount(&server).await;
    Mock::given(path("/oauth/token"))
        .and(body_string_contains("sid=confirmed"))
        .and(body_string_contains("grant_type=phone_confirmation_sid"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token":"signed-in","user_id":7,"expires_in":3600})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.getExchangeToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"response":{"users_exchange_tokens":[{"common_token":"common"}]}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/account.getProfileInfo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"id":7,"first_name":"Test","last_name":"User"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut auth = AuthService::new(client(&server));
    assert!(auth.submit_otp("123456").await.is_err());
    let LoginContinuation::Otp(challenge) = auth.begin("test-login").await.unwrap() else {
        panic!();
    };
    assert_eq!(challenge.code_length, 6);
    assert!(auth.submit_otp("123456").await.unwrap());
    let session = auth.submit_password("test-password").await.unwrap();
    assert_eq!(session.user_id, 7);
    assert_eq!(session.common_token.as_deref(), Some("common"));
    assert_eq!(session.user.unwrap().full_name(), "Test User");
    assert!(auth.profile_preview.is_none());
    assert!(auth.submit_password("test-password").await.is_err());
}

#[derive(Default)]
struct MemoryPersistence(Mutex<Option<Session>>);
impl SessionPersistence for MemoryPersistence {
    fn load(&self) -> Result<Option<Session>> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, session: &Session) -> Result<()> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn session() -> Session {
    Session {
        user_id: 7,
        access_token: "expired".into(),
        common_token: Some("common".into()),
        trusted_hash: None,
        expires_at: None,
        user: None,
    }
}

#[tokio::test]
async fn concurrent_refreshes_share_one_request_and_persist_the_token() {
    let server = MockServer::start().await;
    Mock::given(path("/method/auth.refreshTokens"))
        .and(body_string_contains("exchange_tokens=common"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{"success":[{"access_token":{"token":"renewed","expires_in":3600}}]}}))).expect(1).mount(&server).await;
    let client = client(&server);
    let persistence = Arc::new(MemoryPersistence::default());
    let store = Arc::new(SessionStore::new(persistence.clone()).unwrap());
    store.adopt(session()).unwrap();
    let refresher = SessionTokenRefresher::new(&client, store.clone());
    let (a, b) = tokio::join!(
        refresher.refreshed_token("expired"),
        refresher.refreshed_token("expired")
    );
    assert_eq!(a.unwrap().as_deref(), Some("renewed"));
    assert_eq!(b.unwrap().as_deref(), Some("renewed"));
    assert_eq!(persistence.load().unwrap().unwrap().access_token, "renewed");
    assert!(store.session().unwrap().unwrap().expires_at.is_some());
}

#[tokio::test]
async fn a_refresh_cannot_replace_a_new_account_session() {
    let server = MockServer::start().await;
    let started = Arc::new(tokio::sync::Notify::new());
    let notify = started.clone();
    Mock::given(path("/method/auth.refreshTokens"))
        .respond_with(move |_: &Request| {
            notify.notify_one();
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(20))
                .set_body_json(
                    json!({"response":{"success":[{"access_token":{"token":"renewed"}}]}}),
                )
        })
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);
    let store = Arc::new(SessionStore::new(Arc::new(MemoryPersistence::default())).unwrap());
    store.adopt(session()).unwrap();
    let refresher = SessionTokenRefresher::new(&client, store.clone());
    let (result, ()) = tokio::join!(refresher.refreshed_token("expired"), async {
        started.notified().await;
        let mut next = session();
        next.user_id = 8;
        next.access_token = "other-account".into();
        store.adopt(next).unwrap();
    });
    assert!(result.unwrap().is_none());
    assert_eq!(
        store.session().unwrap().unwrap().access_token,
        "other-account"
    );
}

#[tokio::test]
async fn service_methods_keep_mutation_upload_and_catalog_parameters() {
    let server = MockServer::start().await;
    let playlist = Playlist::new(2, 7, "List");
    for (name, response) in [
        ("audio.removeFromPlaylist", json!(1)),
        (
            "audio.followPlaylist",
            json!({"playlist_id":3,"owner_id":7}),
        ),
        ("audio.deletePlaylist", json!(1)),
        (
            "audio.createPlaylist",
            json!({"id":2,"owner_id":7,"type":0}),
        ),
        ("audio.editPlaylist", json!(1)),
        ("audio.setBroadcast", json!([])),
        (
            "audio.getStreamMixSettings",
            json!({"settings":{"title":"Mix","mix_categories":[]}}),
        ),
        ("audio.getStreamMixAudios", json!([audio()])),
        ("audio.getLyrics", json!({"lyrics":{"text":["line"]}})),
    ] {
        Mock::given(path(format!("/method/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":response})))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(path("/method/photos.saveAudioPlaylistCover"))
        .and(body_string_contains("v=5.272"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"photo":{"unknown":42},"hash":"cover-hash"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.setPlaylistCoverPhoto"))
        .and(body_string_contains("playlist_owner_id=7"))
        .and(body_string_contains("unknown%22%3A42"))
        .and(body_string_contains("v=5.272"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{}})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/catalog.getSection"))
        .and(header("user-agent", CATALOG_USER_AGENT))
        .and(body_string_contains("v=5.272"))
        .and(body_string_contains("start_from=cursor"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"section":{"blocks":[],"next_from":"next"}}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/upload"))
        .and(header("user-agent", vively_core::config::USER_AGENT))
        .and(body_string_contains("filename=\"VKXAudioUpload.mp3\""))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"server":1,"audio":"encoded%2Bblob","hash":"upload-hash"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.save"))
        .and(body_string_contains("audio=encoded%252Bblob"))
        .and(body_string_contains("v=5.272"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":audio()})))
        .expect(1)
        .mount(&server)
        .await;
    let service = AudioService::new(client(&server));
    assert_eq!(
        service
            .remove_from_playlist(&audio(), &playlist, "test")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        service
            .follow_playlist(&playlist, "test")
            .await
            .unwrap()
            .playlist_id,
        Some(3)
    );
    service.delete_playlist(&playlist, "test").await.unwrap();
    service
        .create_playlist(7, "List", "Description", &["7_1".into()], true, "test")
        .await
        .unwrap();
    service
        .edit_playlist(&playlist, "List", "Description", false, "test")
        .await
        .unwrap();
    service.set_broadcast(None, "test").await.unwrap();
    service.stream_mix_settings("common", "test").await.unwrap();
    assert_eq!(
        service
            .stream_mix_audios("common", None, 30, None, "test")
            .await
            .unwrap()
            .len(),
        1
    );
    service.lyrics(&audio(), "test").await.unwrap();
    let cover: SavedPlaylistCover = service.save_cover(7, "raw-response", "test").await.unwrap();
    service
        .set_playlist_cover(&playlist, &cover, "test")
        .await
        .unwrap();
    assert_eq!(
        service
            .catalog_section("section", "cursor", 100, "test")
            .await
            .unwrap()
            .section
            .next_from
            .as_deref(),
        Some("next")
    );
    let ticket = service
        .upload_audio(
            vec![1, 2, 3],
            format!("{}/upload", server.uri()).parse().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ticket.audio, "encoded%2Bblob");
    service
        .save_audio(&ticket, "Artist", "Track", "test")
        .await
        .unwrap();
    assert!(service.tracks_by_id(&[], "test").await.unwrap().is_empty());
}

#[tokio::test]
async fn service_read_requests_use_direct_methods_and_page_arguments() {
    let server = MockServer::start().await;
    Mock::given(path("/method/execute"))
        .respond_with(|request: &Request| {
            let fields = form(request);
            assert_eq!(fields["access_token"], "test-token");
            assert_eq!(fields["v"], "5.272");
            let code = &fields["code"];
            let response = if code.contains("var catalogs =") {
                assert_eq!(request.headers["user-agent"], CATALOG_USER_AGENT);
                json!({"section":{"blocks":[]}})
            } else {
                assert!(code.contains("res.audios ="));
                assert_eq!(fields["access_key"], "playlist-key");
                assert_eq!(fields["offset"], "4");
                assert_eq!(fields["count"], "50");
                json!({"audios":{"count":1,"items":[audio()]}})
            };
            ResponseTemplate::new(200).set_body_json(json!({"response":response}))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.get"))
        .respond_with(|request: &Request| {
            let fields = form(request);
            assert_eq!(fields["owner_id"], "7");
            assert_eq!(fields["offset"], "4");
            assert_eq!(fields["v"], "5.272");
            if fields.contains_key("playlist_id") {
                assert_eq!(fields["access_key"], "playlist-key");
            } else {
                assert!(!fields.contains_key("access_key"));
            }
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"count":1,"items":[audio()]}}))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.getById"))
        .respond_with(|request: &Request| {
            let count = form(request)["audios"].split(',').count();
            assert!(count == 100 || count == 5);
            ResponseTemplate::new(200).set_body_json(json!({"response":[audio()]}))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/method/catalog.getAudioArtist"))
        .and(body_string_contains("artist_id=artist-id"))
        .and(body_string_contains("need_blocks=1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"response":{"artists":[],"audios":[],"playlists":[],"catalog":{"sections":[]}}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/catalog.getAudioSearch"))
        .and(body_string_contains("query=query%20%2B"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"catalog":{"sections":[]}}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.getRecommendations"))
        .and(body_string_contains("target_audio=7_1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"count":1,"items":[audio()]}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/catalog.getAudio"))
        .and(header("user-agent", CATALOG_USER_AGENT))
        .and(body_string_contains("url=https"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{"catalog":{"default_section":"main","sections":[{"id":"other","title":"Other"},{"id":"main","title":"Main","blocks":[]}]}}})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/audio.getPlaylists"))
        .and(body_string_contains("owner_id=7"))
        .and(body_string_contains("offset=4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{"count":1,"items":[{"id":2,"owner_id":9,"type":0,"original":{"owner_id":5,"playlist_id":3}}]}})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/users.get"))
        .and(body_string_contains("user_ids=5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"response":[{"id":5,"first_name":"Playlist","last_name":"Author"}]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/friends.get")).and(body_string_contains("fields=photo_base"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":{"count":1,"items":[{"id":8,"first_name":"Test","last_name":"Friend"}]}}))).expect(1).mount(&server).await;
    let service = AudioService::new(client(&server));
    let page = Page {
        offset: 4,
        count: 50,
    };
    assert_eq!(
        service
            .library(7, page, "test-token")
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        service.friends(page, "test-token").await.unwrap().items[0].id,
        8
    );
    service
        .tracks_by_id(
            &(0..105).map(|i| format!("7_{i}")).collect::<Vec<_>>(),
            "test-token",
        )
        .await
        .unwrap();
    let playlists = service.playlists(7, page, "test-token").await.unwrap();
    assert_eq!(playlists.items[0].owner_id, 7);
    assert_eq!(playlists.items[0].author_name(), Some("Playlist Author"));
    let mut playlist = Playlist::new(2, 7, "List");
    playlist.access_key = Some("playlist-key".into());
    service
        .playlist_contents(&playlist, page, "test-token")
        .await
        .unwrap();
    service
        .artist_card("artist-id", "test-token")
        .await
        .unwrap();
    service.audio_home("test-token").await.unwrap();
    service.search_audio("query +", "test-token").await.unwrap();
    assert_eq!(
        service
            .catalog_page("https://vk.com/audio", "test-token")
            .await
            .unwrap()
            .section
            .title
            .as_deref(),
        Some("Main")
    );
    service
        .playlist_tracks_page(&playlist, page, "test-token")
        .await
        .unwrap();
    service
        .recommendations("7_1", 7, page, "test-token")
        .await
        .unwrap();
}

#[tokio::test]
async fn unliking_a_foreign_track_removes_the_users_copy() {
    let server = MockServer::start().await;
    let sequence = Arc::new(AtomicUsize::new(0));
    let seen = sequence.clone();
    Mock::given(path("/method/execute"))
        .respond_with(move |request: &Request| {
            let fields = form(request);
            match seen.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    assert!(fields["code"].contains("API.audio.add("));
                    assert_eq!(fields["owner_id"], "9");
                    assert_eq!(fields["audio_id"], "1");
                    ResponseTemplate::new(200).set_body_json(json!({"response":99}))
                }
                1 => {
                    assert!(fields["code"].contains("API.audio.delete("));
                    assert_eq!(fields["owner_id"], "7");
                    assert_eq!(fields["audio_id"], "99");
                    ResponseTemplate::new(200).set_body_json(json!({"response":1}))
                }
                _ => {
                    assert!(fields["code"].contains("API.audio.restore("));
                    ResponseTemplate::new(200).set_body_json(json!({"response":1}))
                }
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    let mut library = MusicLibrary::new(AudioService::new(client(&server)), 7);
    let mut foreign = audio();
    foreign.owner_id = 9;
    assert!(library.toggle_like(&foreign, "test-token").await.unwrap());
    assert!(library.is_liked(&foreign));
    assert!(!library.toggle_like(&foreign, "test-token").await.unwrap());
    assert!(!library.is_liked(&foreign));
    assert!(library.toggle_like(&audio(), "test-token").await.unwrap());
    assert_eq!(library.tracks().len(), 1);
}

#[tokio::test]
async fn sms_authentication_dispatches_an_otp_and_profile_failures_do_not_block_login() {
    let server = MockServer::start().await;
    Mock::given(path("/oauth/get_anonym_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token":"anonymous"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.validateAccount"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"response":{"sid":"validated","next_step":{"verification_method":"sms"}}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/ecosystem.sendOtpSms"))
        .and(body_string_contains("sid=validated"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"response":{"code_length":4,"info":"masked"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token":"signed-in","user_id":7})),
        )
        .expect(1)
        .mount(&server)
        .await;
    for name in ["auth.getExchangeToken", "account.getProfileInfo"] {
        Mock::given(path(format!("/method/{name}")))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
    }
    let mut auth = AuthService::new(client(&server));
    let LoginContinuation::Otp(otp) = auth.begin("test-login").await.unwrap() else {
        panic!();
    };
    assert_eq!(otp.code_length, 4);
    assert_eq!(otp.masked_destination.as_deref(), Some("masked"));
    let session = auth.submit_password("test-password").await.unwrap();
    assert_eq!(session.access_token, "signed-in");
    assert!(session.common_token.is_none());
    assert!(session.user.is_none());
}

#[tokio::test]
async fn feature_methods_send_vkx_parameters() {
    let server = MockServer::start().await;
    let checks: [(&str, &[&str], serde_json::Value); 8] = [
        ("audio.addDislike", &["audio_ids=7_1"], json!(1)),
        ("audio.removeDislike", &["audio_ids=7_1"], json!(1)),
        (
            "audio.getRelatedArtistsById",
            &["artist_id=artist-id", "count=10"],
            json!({"artists":[{"id":"a2","name":"Other"}]}),
        ),
        (
            "audio.followArtist",
            &["artist_id=artist-id", "user_id=9"],
            json!(1),
        ),
        ("audio.followOwner", &["owner_id=8"], json!(1)),
        (
            "audio.reorderInPlaylist",
            &[
                "playlist_id=2",
                "owner_id=7",
                "actions=%5B%5B7%2C1%2C3%5D%5D",
            ],
            json!(1),
        ),
        (
            "audio.savePlaylistAsCopy",
            &["playlist_id=2", "owner_id=7"],
            json!({"id":5,"owner_id":9,"type":"ugc"}),
        ),
        (
            "execute",
            &["owner_id=8", "query=word", "searchPlaylists"],
            json!({"playlists":{"count":1,"items":[{"id":3,"owner_id":8,"type":"ugc"}]},"audios":[audio()]}),
        ),
    ];
    for (name, fragments, response) in checks {
        let mut mock =
            Mock::given(path(format!("/method/{name}"))).and(body_string_contains("v=5.272"));
        for fragment in fragments {
            mock = mock.and(body_string_contains(*fragment));
        }
        mock.respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":response})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let service = AudioService::new(client(&server));
    let playlist = Playlist::new(2, 7, "List");
    service.add_dislike(&audio(), "test").await.unwrap();
    service.remove_dislike(&audio(), "test").await.unwrap();
    assert_eq!(
        service.related_artists("artist-id", "test").await.unwrap()[0].name,
        "Other"
    );
    service
        .set_artist_followed("artist-id", 9, true, "test")
        .await
        .unwrap();
    service.set_owner_followed(8, true, "test").await.unwrap();
    service
        .move_in_playlist(&playlist, &audio(), 3, "test")
        .await
        .unwrap();
    assert_eq!(
        service
            .save_playlist_copy(&playlist, "test")
            .await
            .unwrap()
            .kind,
        vively_core::models::PlaylistKind::Playlist
    );
    let found = service.search_in_profile(8, "word", "test").await.unwrap();
    assert_eq!(found.playlists.items.len(), 1);
    assert_eq!(found.audios.len(), 1);
}

#[tokio::test]
async fn library_writes_keep_the_5_199_response_format() {
    let server = MockServer::start().await;
    Mock::given(path("/method/execute"))
        .and(body_string_contains("API.audio.add"))
        .and(body_string_contains("v=5.199"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response":456})))
        .expect(1)
        .mount(&server)
        .await;
    let service = AudioService::new(client(&server));
    assert_eq!(service.add_to_library(&audio(), "test").await.unwrap(), 456);
}

#[tokio::test]
async fn lyrics_provider_prefers_musixmatch_synced_over_vk_plain_and_genius_over_musixmatch_plain() {
    let vk_server = MockServer::start().await;
    let genius_server = MockServer::start().await;
    let mxm_server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let token_path = tmp.path().join("mxm_token.json");

    Mock::given(path("/token.get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": {
                "header": {"status_code": 200},
                "body": {"user_token": "live-mxm-token-abc"}
            }
        })))
        .expect(1)
        .mount(&mxm_server)
        .await;

    let synced_body = json!([
        {"text": "Synced line", "time": {"total": 2.0}},
        {"text": "", "time": {"total": 8.0}}
    ])
    .to_string();
    Mock::given(path("/macro.subtitles.get"))
        .respond_with(move |req: &Request| {
            let query: BTreeMap<_, _> = req.url.query_pairs().into_owned().collect();
            assert_eq!(query["usertoken"], "live-mxm-token-abc");
            if query["q_track"] == "TrackSynced" {
                ResponseTemplate::new(200).set_body_json(json!({
                    "message": {
                        "header": {"status_code": 200},
                        "body": {
                            "macro_calls": {
                                "matcher.track.get": {"message": {"header": {"status_code": 200}, "body": {"track": {"instrumental": 0}}}},
                                "track.subtitles.get": {"message": {"header": {"status_code": 200}, "body": {"subtitle_list": [{"subtitle": {"subtitle_body": synced_body}}]}}}
                            }
                        }
                    }
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(json!({
                    "message": {
                        "header": {"status_code": 200},
                        "body": {
                            "macro_calls": {
                                "matcher.track.get": {"message": {"header": {"status_code": 200}, "body": {"track": {"instrumental": 0}}}},
                                "track.subtitles.get": {"message": {"header": {"status_code": 404}, "body": []}},
                                "track.lyrics.get": {"message": {"header": {"status_code": 200}, "body": {"lyrics": {"instrumental": 0, "lyrics_body": "MXM plain"}}}}
                            }
                        }
                    }
                }))
            }
        })
        .expect(2)
        .mount(&mxm_server)
        .await;

    Mock::given(path("/method/audio.getLyrics"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"lyrics": {"text": ["VK plain line"]}}
        })))
        .expect(1)
        .mount(&vk_server)
        .await;

    Mock::given(path("/search/song"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "sections": [{
                    "type": "song",
                    "hits": [{
                        "type": "song",
                        "result": {
                            "id": 99,
                            "title": "TrackPlain",
                            "artist_names": "Artist",
                            "lyrics_state": "complete",
                            "instrumental": false
                        }
                    }]
                }]
            }
        })))
        .expect(1)
        .mount(&genius_server)
        .await;

    Mock::given(path("/songs/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "song": {
                    "instrumental": false,
                    "lyrics": {"plain": "[Verse 1]\nGenius clean line"}
                }
            }
        })))
        .expect(1)
        .mount(&genius_server)
        .await;

    let mut provider = LyricsProvider::with_services(
        AudioService::new(client(&vk_server)),
        GeniusService::with_base_url(genius_server.uri()).unwrap(),
        MusixmatchService::with_base_url(mxm_server.uri())
            .unwrap()
            .with_token_path(&token_path),
    );

    let mut track_with_vk_plain = audio();
    track_with_vk_plain.id = 10;
    track_with_vk_plain.title = "TrackSynced".into();
    track_with_vk_plain.has_lyrics = Some(true);
    let res1 = provider
        .load(&track_with_vk_plain, Some("vk-token"), true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(res1.source, LyricsSource::Musixmatch);
    assert!(matches!(res1.content, LyricsContent::Synced(_)));
    assert!(token_path.exists());

    let mut track_no_vk = audio();
    track_no_vk.id = 11;
    track_no_vk.title = "TrackPlain".into();
    track_no_vk.has_lyrics = Some(false);
    let res2 = provider
        .load(&track_no_vk, Some("vk-token"), true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(res2.source, LyricsSource::Genius);
    assert_eq!(
        res2.content,
        LyricsContent::Plain(vec!["Genius clean line".into()])
    );
}

#[tokio::test]
async fn musixmatch_rejects_zero_honeypot_token_and_falls_back_to_genius() {
    let vk_server = MockServer::start().await;
    let genius_server = MockServer::start().await;
    let mxm_server = MockServer::start().await;

    Mock::given(path("/token.get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": {
                "header": {"status_code": 200},
                "body": {"user_token": "00000000000000000000000000000000000000000000000000000000"}
            }
        })))
        .expect(1)
        .mount(&mxm_server)
        .await;

    Mock::given(path("/search/song"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "sections": [{
                    "type": "song",
                    "hits": [{
                        "type": "song",
                        "result": {
                            "id": 77,
                            "title": "Track",
                            "artist_names": "Artist",
                            "lyrics_state": "complete",
                            "instrumental": false
                        }
                    }]
                }]
            }
        })))
        .expect(1)
        .mount(&genius_server)
        .await;

    Mock::given(path("/songs/77"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "song": {
                    "instrumental": false,
                    "lyrics": {"plain": "[Chorus]\nFallback lyric"}
                }
            }
        })))
        .expect(1)
        .mount(&genius_server)
        .await;

    let mut provider = LyricsProvider::with_services(
        AudioService::new(client(&vk_server)),
        GeniusService::with_base_url(genius_server.uri()).unwrap(),
        MusixmatchService::with_base_url(mxm_server.uri()).unwrap(),
    );

    let res = provider.load(&audio(), None, true).await.unwrap().unwrap();
    assert_eq!(res.source, LyricsSource::Genius);
    assert_eq!(
        res.content,
        LyricsContent::Plain(vec!["Fallback lyric".into()])
    );
}

#[tokio::test]
async fn qr_authentication_polls_status_validates_code_and_builds_session() {
    let server = MockServer::start().await;
    Mock::given(path("/oauth/get_anonym_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "anon-qr"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.getAuthCode"))
        .respond_with(|request: &Request| {
            let fields = form(request);
            assert_eq!(fields["access_token"], "anon-qr");
            assert_eq!(fields["device_name"], "Vively (macOS)");
            assert_eq!(fields["force_regenerate"], "1");
            assert_eq!(fields["auth_code_flow"], "0");
            ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "auth_code": "AbC123",
                    "auth_hash": "qr-hash-1",
                    "auth_id": "qr-id-1",
                    "auth_url": "https://qr.vk.ru/ca?q=AbC123",
                    "expires_in": 1800000000
                }
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let poll_step = Arc::new(AtomicUsize::new(0));
    let poll_seen = poll_step.clone();
    Mock::given(path("/method/auth.checkAuthCode"))
        .respond_with(move |request: &Request| {
            let fields = form(request);
            assert_eq!(fields["access_token"], "anon-qr");
            assert_eq!(fields["auth_hash"], "qr-hash-1");
            assert_eq!(fields["web_auth"], "0");
            assert_eq!(fields["scope"], "all");
            match poll_seen.fetch_add(1, Ordering::SeqCst) {
                0 => ResponseTemplate::new(200).set_body_json(json!({
                    "response": {"status": 0, "expires_in": 1800000000, "user_info": []}
                })),
                1 => ResponseTemplate::new(200).set_body_json(json!({
                    "response": {
                        "status": 1,
                        "user_info": {"first_name": "Pavel", "last_name": "Durov", "photo_200": ""}
                    }
                })),
                2 => ResponseTemplate::new(200).set_body_json(json!({
                    "response": {"status": 5}
                })),
                _ => ResponseTemplate::new(200).set_body_json(json!({
                    "response": {
                        "status": 2,
                        "access_token": "qr-access-token",
                        "user_id": "42",
                        "expires_in": 86400,
                        "trusted_hash": "qr-trusted"
                    }
                })),
            }
        })
        .expect(4)
        .mount(&server)
        .await;

    let val_step = Arc::new(AtomicUsize::new(0));
    let val_seen = val_step.clone();
    Mock::given(path("/method/auth.validateAuthCode"))
        .respond_with(move |request: &Request| {
            let fields = form(request);
            assert_eq!(fields["auth_hash"], "qr-hash-1");
            match val_seen.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    assert_eq!(fields["validation_code"], "000000");
                    ResponseTemplate::new(200).set_body_json(json!({"response": {"status": 1}}))
                }
                _ => {
                    assert_eq!(fields["validation_code"], "654321");
                    ResponseTemplate::new(200).set_body_json(json!({"response": {"status": 0}}))
                }
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(path("/method/auth.getExchangeToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"users_exchange_tokens": [{"common_token": "qr-common"}]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/account.getProfileInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"id": 42, "first_name": "Pavel", "last_name": "Durov"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut auth = AuthService::new(client(&server));
    assert!(auth.check_qr_auth().await.is_err());

    let qr = auth.start_qr_auth("Vively (macOS)").await.unwrap();
    assert_eq!(qr.auth_code, "AbC123");
    assert_eq!(qr.auth_url, "https://qr.vk.ru/ca?q=AbC123");
    assert_eq!(qr.expires_at, Some(1800000000));

    assert!(matches!(auth.check_qr_auth().await.unwrap(), QrAuthStatus::Pending));

    let QrAuthStatus::Scanned { user_info } = auth.check_qr_auth().await.unwrap() else {
        panic!("expected Scanned");
    };
    assert_eq!(
        user_info.unwrap().display_name().as_deref(),
        Some("Pavel Durov")
    );

    assert!(matches!(
        auth.check_qr_auth().await.unwrap(),
        QrAuthStatus::NeedConfirmationCode
    ));
    assert_eq!(
        auth.validate_qr_code("000000").await.unwrap(),
        QrValidationResult::IncorrectCode
    );
    assert_eq!(
        auth.validate_qr_code("654321").await.unwrap(),
        QrValidationResult::Accepted
    );

    let QrAuthStatus::Authorized(session) = auth.check_qr_auth().await.unwrap() else {
        panic!("expected Authorized");
    };
    assert_eq!(session.user_id, 42);
    assert_eq!(session.access_token, "qr-access-token");
    assert_eq!(session.common_token.as_deref(), Some("qr-common"));
    assert_eq!(session.trusted_hash.as_deref(), Some("qr-trusted"));
    assert_eq!(session.user.unwrap().full_name(), "Pavel Durov");
    assert!(auth.check_qr_auth().await.is_err());
}

#[tokio::test]
async fn qr_authentication_handles_declined_expired_and_revoke() {
    let server = MockServer::start().await;
    Mock::given(path("/oauth/get_anonym_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "anon-qr"})))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.getAuthCode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "auth_code": "XYZ999",
                "auth_hash": "qr-hash-2",
                "auth_url": "https://qr.vk.ru/ca?q=XYZ999",
                "expires_in": 180
            }
        })))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.checkAuthCode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"status": 3}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/method/auth.revokeAuthCode"))
        .respond_with(|request: &Request| {
            let fields = form(request);
            assert_eq!(fields["auth_hash"], "qr-hash-2");
            assert_eq!(fields["access_token"], "anon-qr");
            ResponseTemplate::new(200).set_body_json(json!({"response": {"status": 1}}))
        })
        .expect(1)
        .mount(&server)
        .await;

    let mut auth = AuthService::new(client(&server));
    let qr = auth.start_qr_auth("Vively").await.unwrap();
    assert!(qr.expires_at.is_some());
    assert!(matches!(auth.check_qr_auth().await.unwrap(), QrAuthStatus::Declined));
    assert!(auth.check_qr_auth().await.is_err());

    let _ = auth.start_qr_auth("Vively").await.unwrap();
    auth.revoke_qr_auth().await.unwrap();
    assert!(auth.check_qr_auth().await.is_err());
}
