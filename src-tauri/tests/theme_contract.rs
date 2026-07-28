use codex_assistant_lib::theme::{
    apply_theme_on_pages_for_version, bundled_theme_packs, select_theme_adapter,
    theme_application_source, theme_application_source_with_asset,
    theme_page_classification_source, theme_restore_source, theme_verification_source,
    validate_theme_pack, RightsStatus, ThemeBackdrop, ThemeCategory, ThemeEngineError,
    ThemeScriptRegistration,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[test]
fn bundled_themes_are_declarative_project_owned_and_pass_the_rights_gate() {
    let packs = bundled_theme_packs();
    assert_eq!(
        packs
            .iter()
            .map(|pack| pack.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "wisteria-bride",
            "mint-gentleman",
            "iris-gentleman",
            "crimson-palace",
            "verdant-fairy",
            "desert-prince",
            "oasis-prince",
            "sakura-moon",
            "seaside-blue",
            "autumn-wuxia",
            "meteor-evening",
            "violet-blade",
            "fuji-autumn",
            "spring-street",
        ]
    );
    assert!(packs
        .iter()
        .any(|pack| pack.category == ThemeCategory::OriginalCharacter));
    for retired in [
        "aurora-grid",
        "observatory-muse",
        "gothic-horizon",
        "roseglass-atelier",
        "blush-circuit",
        "fortune-foundry",
        "crimson-relay",
        "crystal-daylight",
        "pocket-cosmos",
        "violet-afterdark",
        "cyan-chorus",
        "noir-stage",
    ] {
        assert!(
            !packs.iter().any(|pack| pack.id == retired),
            "retired bundled theme remains: {retired}"
        );
    }
    for pack in packs {
        validate_theme_pack(&pack, true).expect("bundled theme rights gate");
        assert_eq!(pack.rights.status, RightsStatus::Verified);
        assert!(pack.rights.commercial_redistribution);
        assert!(!pack.rights.attribution.is_empty());
        assert!(pack.assets.iter().all(|asset| {
            asset.sha256.len() == 64
                && asset
                    .sha256
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        }));
        let serialized = serde_json::to_string(&pack).unwrap();
        for forbidden in [
            "<script",
            "javascript:",
            "http://",
            "https://",
            "powershell",
            "Arina Hashimoto",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "forbidden pack content: {forbidden}"
            );
        }
    }
}

#[test]
fn wisteria_bride_is_an_offline_bundled_theme_that_reaches_the_real_engine() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| pack.id == "wisteria-bride")
        .expect("approved Wisteria Bride theme");

    assert_eq!(pack.name, "紫藤花嫁");
    assert_eq!(pack.preview_path, "/themes/wisteria-bride.webp");
    assert_eq!(pack.rights.status, RightsStatus::Verified);
    assert!(pack.rights.commercial_redistribution);

    let source = theme_application_source(&pack).expect("offline image theme source");
    assert!(source.contains("data:image/webp;base64,"));
    assert!(!source.contains("http://"));
    assert!(!source.contains("https://"));
}

#[test]
fn theme_engine_generates_only_owned_dom_presentation_and_exact_restore() {
    let mut pack = bundled_theme_packs().remove(0);
    pack.backdrop = ThemeBackdrop::Gradient {
        angle: 135,
        colors: ["#07111f".into(), "#18204b".into(), "#0b4d5f".into()],
    };
    let source = theme_application_source(&pack).expect("validated engine source");
    for required in [
        "__codexAssistantThemeV1",
        "data-codex-assistant-theme",
        "main.main-surface",
        "aside.app-shell-left-panel",
        "prefers-reduced-motion",
    ] {
        assert!(
            source.contains(required),
            "missing engine contract: {required}"
        );
    }
    for forbidden in [
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "eval(",
        "localStorage",
        "sessionStorage",
        "navigator.clipboard",
        "innerHTML",
        "innerText",
        ".textContent",
        "http://",
        "https://",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden engine API: {forbidden}"
        );
    }
    let restore = theme_restore_source();
    assert!(restore.contains("__codexAssistantThemeV1"));
    assert!(restore.contains("remove"));
    assert!(!restore.contains("querySelectorAll('*')"));
}

#[test]
fn page_classification_is_positive_bounded_and_sensitive_first() {
    let classification = theme_page_classification_source();
    for class in [
        "compatible-main",
        "compatible-shell",
        "utility",
        "sensitive",
        "unknown-build",
        "invalid-target",
    ] {
        assert!(classification.contains(class), "missing page class {class}");
    }
    for required in [
        "main.main-surface",
        "aside.app-shell-left-panel",
        ".composer-surface-chrome",
        "input[type=\"password\"]",
        "data-permission-screen",
        "data-security-prompt",
        "data-codex-utility-page",
        "elementFromPoint",
    ] {
        assert!(
            classification.contains(required),
            "missing bounded page evidence {required}"
        );
    }
    assert!(!classification.contains("location.href"));

    let source = theme_application_source(&bundled_theme_packs()[0]).expect("theme source");
    assert!(source.contains("MutationObserver"));
    assert!(source.contains("style.disabled"));
    assert!(source
        .contains("pageClass=>pageClass===\"compatible-main\"||pageClass===\"compatible-shell\""));
    assert!(theme_restore_source().contains("data-codex-assistant-page-class"));
}

#[test]
fn adapter_registry_accepts_only_the_reviewed_official_build_family() {
    assert!(select_theme_adapter("26.715.3651.0").is_some());
    assert!(select_theme_adapter("26.721.4979.0").is_some());
    assert!(select_theme_adapter("26.799.9999.0").is_some());
    assert!(select_theme_adapter("25.721.4979.0").is_none());
    assert!(select_theme_adapter("26.800.1.0").is_none());
    assert!(select_theme_adapter("not-a-version").is_none());
}

#[test]
fn theme_css_never_recolors_or_overlays_codex_semantic_content() {
    for pack in bundled_theme_packs() {
        let source = theme_application_source(&pack).expect("theme source");
        for forbidden in [
            "--color-token-foreground:",
            "--color-token-text-primary:",
            "--color-token-text-secondary:",
            "--color-token-text-tertiary:",
            "fill:currentColor!important",
            r#"button[class~=\"bg-token-foreground\"]"#,
            r#"button[class~=\"bg-token-button-background\"]"#,
            "html,body,#root{background:transparent",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} contains unsafe content override: {forbidden}",
                pack.id
            );
        }
        assert!(source.contains("body::before"));
        assert!(source.contains("pointer-events:none"));
        assert!(source.contains("[data-codex-assistant-welcome-action]{pointer-events:auto"));
        assert!(!source.contains("body::before{pointer-events:auto"));
        assert!(!source.contains("body::after{pointer-events:auto"));
        assert!(!source.contains("z-index:2147483647"));
    }
}

#[test]
fn every_bundled_theme_uses_a_verified_main_visual_and_shared_detail_system() {
    for pack in bundled_theme_packs() {
        assert!(
            matches!(pack.backdrop, ThemeBackdrop::Image { .. }),
            "{} must use its rights-cleared raster preview as the main visual",
            pack.id
        );
        let source = theme_application_source(&pack).expect("theme source");
        for required in [
            "data:image/",
            "[data-user-message-bubble",
            "[aria-current=",
            "from-token-main-surface-primary",
            "border-token-border",
            "scrollbar-color",
        ] {
            assert!(
                source.contains(required),
                "{} is missing shared detail contract {required}",
                pack.id
            );
        }
        let verification = theme_verification_source(&pack).expect("verification source");
        assert!(
            verification.contains("imageVisibility>=0.4")
                || verification.contains("split(\"data:image/\")"),
            "{} must verify a visibly present main visual",
            pack.id
        );
        assert!(
            !source.contains("main.main-surface button{"),
            "{} must preserve filled-button contrast instead of forcing generic button text",
            pack.id
        );
        assert!(!source.contains("--color-token-dropdown-background"));
        assert!(!source.contains("--codex-assistant-theme-action-text"));
        assert!(!source.contains(r#"button[class~=\"bg-token-foreground\"]"#));
        assert!(!source.contains("#root{background:transparent!important;color:"));
    }
}

#[test]
fn every_theme_uses_one_fixed_cover_layer_without_distortion() {
    for pack in bundled_theme_packs() {
        let source = theme_application_source(&pack).expect("theme source");
        let embedded_images = source.matches("data:image/").count();
        assert_eq!(
            embedded_images, 1,
            "{} must load one full-window background image",
            pack.id
        );
        assert!(
            source.contains("background-size:cover")
                && source.contains("background-repeat:no-repeat")
                && source.contains("background-position:var(--codex-assistant-theme-focal-x) var(--codex-assistant-theme-focal-y)"),
            "{} must fill the window with a focal-aware cover image",
            pack.id
        );
        assert!(
            source.contains("body::before")
                && source.contains("position:fixed")
                && source.contains("inset:0")
                && source.contains("pointer-events:none"),
            "{} must render one stable full-window background layer",
            pack.id
        );
        assert!(
            !source.contains("background-size:100% 100%;")
                && !source.contains("background-repeat:repeat")
                && !source.contains("filter:blur(2px)!important"),
            "{} must preserve aspect ratio without tiling or blurring the artwork",
            pack.id
        );
        for required in [
            "bg-token-button-background-secondary",
            ":active",
            "input:focus-visible",
            "textarea:focus-visible",
        ] {
            assert!(source.contains(required), "{} lacks {required}", pack.id);
        }
        assert!(
            contrast_ratio(&pack.palette.text, &pack.palette.surface) >= 4.5,
            "{} palette misses WCAG AA body contrast",
            pack.id
        );
        assert!(
            contrast_ratio(&pack.palette.text, &pack.palette.surface_strong) >= 4.5,
            "{} palette misses WCAG AA strong-surface contrast",
            pack.id
        );
    }
}

#[test]
fn all_theme_backgrounds_are_crisp_and_focal_responsive() {
    for pack in bundled_theme_packs() {
        let source = theme_application_source(&pack).expect("theme source");
        assert!(source.contains("filter:brightness(0.92) saturate(1.08) contrast(1.04)"));
        assert!(source.contains("@media(max-width:1200px)"));
        assert!(source.contains("@media(min-aspect-ratio:21/9)"));
        assert!(!source.contains("body::before{filter:blur("));
    }
}

#[test]
fn image_themes_use_one_light_dark_scrim_without_uniform_white_opacity() {
    let packs = bundled_theme_packs();
    let theme = packs
        .iter()
        .find(|pack| pack.id == "spring-street")
        .expect("image theme");
    let source = theme_application_source(theme).expect("theme source");

    assert!(
        source.contains("body::after")
            && source.contains("linear-gradient(90deg,rgba(18,12,17,0.18) 0%,rgba(18,12,17,0.06) 42%,rgba(18,12,17,0.02) 72%,rgba(18,12,17,0.12) 100%)"),
        "the artwork must keep detail beneath one restrained dark balance layer"
    );
    assert!(source.contains("body::after{content:\\\"\\\";position:fixed;inset:0"));
    assert!(source.contains("body::after") && source.contains("pointer-events:none"));
    assert!(!source.contains("body::before{opacity:"));
    assert!(!source.contains("body::after{opacity:"));
}

#[test]
fn main_application_surfaces_stay_transparent_over_the_global_backdrop() {
    let portrait = bundled_theme_packs()
        .into_iter()
        .find(|pack| pack.id == "spring-street")
        .expect("portrait theme");
    let source = theme_application_source(&portrait).expect("portrait source");

    assert!(
        source.contains("body main.main-surface,body main[role=\\\"main\\\"]")
            && source.contains("background:transparent!important"),
        "the primary task surface must not place a white wash over the artwork"
    );
    assert!(
        !source.contains("rgba(255,250,251,0.30)") && !source.contains("rgba(250,247,248,0.78)"),
        "legacy full-page white scrims must be removed"
    );
}

#[test]
fn every_image_theme_uses_focal_aware_responsive_cover_positioning() {
    for pack in bundled_theme_packs() {
        let source = theme_application_source(&pack).expect("theme source");
        assert_eq!(
            source.matches("data:image/").count(),
            1,
            "{} must use exactly one complete source image",
            pack.id
        );
        assert!(
            source.contains("background-size:cover")
                && source.contains("@media(max-width:1200px)")
                && source.contains("@media(min-aspect-ratio:21/9)"),
            "{} must cover normal, narrow and ultrawide windows without stretching",
            pack.id
        );
        assert!(
            source.contains("--codex-assistant-theme-focal-x")
                && source.contains("--codex-assistant-theme-focal-y"),
            "{} must preserve its reviewed subject focal point",
            pack.id
        );
        assert!(
            !source.contains("background-size:100% 100%;"),
            "{} must never stretch the approved source image",
            pack.id
        );
    }
}

fn contrast_ratio(foreground: &str, background: &str) -> f64 {
    fn luminance(hex: &str) -> f64 {
        let channel = |offset: usize| {
            let value = u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap() as f64 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
    }
    let first = luminance(foreground);
    let second = luminance(background);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn image_theme_emits_a_parseable_data_url_without_app_protocol_fallback() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Image { .. }))
        .expect("image theme");
    let source = theme_application_source(&pack).expect("theme source");

    assert!(source.contains(r#"url(\"data:image/"#));
    assert!(!source.contains(r#"url(\\\"data:image/"#));
    let verification = theme_verification_source(&pack).expect("verification source");
    assert!(verification.contains("data:image/"));
    assert!(verification.contains("app://"));
}

#[test]
fn image_theme_preserves_backdrop_visibility_with_a_dark_glass_frame() {
    let mut pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Image { .. }))
        .expect("image theme");
    pack.effects.surface_opacity = 78;
    let source = theme_application_source(&pack).expect("theme source");

    for required in [
        "--codex-assistant-theme-chrome:rgba(31,21,28,0.46)",
        "--codex-assistant-theme-chrome-strong:rgba(35,23,31,0.58)",
        "--codex-assistant-theme-chrome-text:rgba(255,248,251,0.94)",
        "--codex-assistant-theme-reading:rgba(255,250,252,0.76)",
        "--codex-assistant-theme-rose:#C67D91",
        "backdrop-filter:blur(18px) saturate(130%)",
        "body .app-header-tint",
        "body [data-codex-output-panel",
        "body [class*=\\\"origin-top-right\\\"][class*=\\\"pointer-events-none\\\"]>[class*=\\\"pointer-events-auto\\\"]>[class*=\\\"bg-token-dropdown-background\\\"]",
        "body main .composer-surface-chrome",
        "border-radius:16px",
        "[data-user-message-bubble",
        "border-radius:14px",
        "data-codex-assistant-theme-welcome",
        "data-codex-assistant-welcome-action",
    ] {
        assert!(
            source.contains(required),
            "missing unified dark-glass contract: {required}"
        );
    }
    assert!(source.contains("background:rgba(31,21,28,0.46)!important"));
    assert!(source.contains("background:rgba(29,22,28,0.72)!important"));
    assert!(source.contains("transition:background-color 160ms ease"));
    assert!(source.contains("scrollbar-width:thin"));
    let verification = theme_verification_source(&pack).expect("verification source");
    assert!(verification.contains("backdropStyle.backgroundSize===\"cover\""));
}

#[test]
fn composer_theme_uses_one_native_glass_surface_without_an_inner_editor_shell() {
    let pack = bundled_theme_packs().remove(0);
    let source = theme_application_source(&pack).expect("theme source");

    assert!(source.contains("body main .composer-surface-chrome"));
    assert!(
        source.contains(
            "body main.main-surface .composer-surface-chrome[class*=\\\"bg-token-input-background\\\"]"
        ),
        "the terminal composer override must beat Codex's dropdown/input utility classes"
    );
    assert!(source.contains("border-width:1px!important"));
    assert!(!source.contains("contenteditable{background"));
}

#[test]
fn current_codex_message_and_output_markup_receive_local_reading_materials() {
    let pack = bundled_theme_packs().remove(0);
    let source = theme_application_source(&pack).expect("theme source");

    assert!(
        source.contains(
            "[data-content-search-unit-key$=\\\":assistant\\\"]>[data-response-annotation-target]"
        ),
        "current assistant messages need a bounded reading card instead of page-wide wash"
    );
    assert!(
        source.contains(
            "[class*=\\\"origin-top-right\\\"] [class*=\\\"bg-token-dropdown-background\\\"] header[class*=\\\"bg-token-dropdown-background\\\"]"
        ),
        "current output-panel section headers must use the dark-glass subgroup material"
    );
    assert!(
        source.contains(
            "[class*=\\\"origin-top-right\\\"][class*=\\\"pointer-events-none\\\"]>[class*=\\\"pointer-events-auto\\\"]>[class*=\\\"bg-token-dropdown-background\\\"] :where(strong,span,p,a,button,[role=\\\"button\\\"])"
        ),
        "current output-panel leaf text must remain legible on dark glass"
    );
    assert!(
        source.contains("[data-local-conversation-item-target-ids]"),
        "current tool calls and file results need a bounded reading card"
    );
}

#[test]
fn theme_details_create_depth_for_headers_cards_composer_and_sidebar() {
    let pack = bundled_theme_packs().remove(0);
    let source = theme_application_source(&pack).expect("theme source");

    for required in [
        "body .app-header-tint,body header[role=\\\"banner\\\"]",
        "background:rgba(31,21,28,0.46)!important",
        "body aside.app-shell-left-panel,body aside[aria-label]",
        "box-shadow:12px 0 30px rgba(10,6,9,0.16)",
        "body main.main-surface [class*=\\\"bg-token-dropdown-background\\\"]",
        "body main .composer-surface-chrome:focus-within",
        "0 0 0 2px rgba(198,125,145,0.20)",
    ] {
        assert!(
            source.contains(required),
            "missing visual detail contract: {required}"
        );
    }
}

#[test]
fn primary_action_keeps_codex_native_foreground_and_fill() {
    let mut pack = bundled_theme_packs().remove(0);
    pack.palette.accent = "#777777".to_owned();

    let source = theme_application_source(&pack).expect("theme source");

    assert!(!source.contains("--codex-assistant-theme-action-text"));
    assert!(!source.contains("button[class~="));
    assert!(!source.contains("fill:currentColor"));
}

#[test]
fn image_theme_rejects_hash_mismatched_runtime_bytes() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Image { .. }))
        .expect("image theme");

    assert_eq!(
        theme_application_source_with_asset(&pack, Some(b"wrong")),
        Err(codex_assistant_lib::theme::ThemeValidationError::InvalidAsset)
    );
}

#[test]
fn bundled_gate_rejects_local_only_or_unreviewed_redistribution() {
    let mut pack = bundled_theme_packs().remove(0);
    pack.rights.status = RightsStatus::LocalOnly;
    assert!(validate_theme_pack(&pack, true).is_err());
    assert!(validate_theme_pack(&pack, false).is_ok());
    pack.rights.commercial_redistribution = false;
    assert!(validate_theme_pack(&pack, true).is_err());
}

#[tokio::test]
async fn theme_apply_uses_verified_page_targets_and_boolean_compatibility_acknowledgement() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"page-1","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/page-1"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let compatibility = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let compatibility: serde_json::Value = serde_json::from_str(&compatibility).unwrap();
        assert_eq!(compatibility["method"], "Runtime.evaluate");
        socket
            .send(Message::Text(
                format!(
                    r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                    compatibility["id"]
                )
                .into(),
            ))
            .await
            .unwrap();
        drop(socket);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..4 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            match index {
                0 => assert_eq!(call["method"], "Page.enable"),
                1 => {
                    assert_eq!(call["method"], "Page.removeScriptToEvaluateOnNewDocument");
                    assert_eq!(call["params"]["identifier"], "stale-script");
                    socket
                        .send(Message::Text(
                            format!(
                                r#"{{"id":{},"error":{{"code":-32000,"message":"No script with given id"}}}}"#,
                                call["id"]
                            )
                            .into(),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
                2 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    assert!(call["params"]["source"]
                        .as_str()
                        .unwrap()
                        .contains("data-codex-assistant-theme"));
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    let expression = call["params"]["expression"].as_str().unwrap();
                    assert!(
                        expression.contains("data-codex-assistant-theme"),
                        "application and verification must share one renderer turn"
                    );
                    assert!(
                        expression.contains("getComputedStyle"),
                        "application and verification must share one renderer turn"
                    );
                    socket
                        .send(Message::Text(
                            format!(
                                r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                                call["id"]
                            )
                            .into(),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
            }
            let result = if index == 2 {
                r#"{"identifier":"theme-script-1"}"#
            } else {
                "{}"
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);
    let previous = [ThemeScriptRegistration {
        target_id: "page-1".to_owned(),
        identifier: "stale-script".to_owned(),
    }];

    let result =
        apply_theme_on_pages_for_version(&endpoint, "26.721.4979.0", &pack, &previous, 1_000)
            .await
            .unwrap();
    assert_eq!(result.applied_pages, 1);
    assert_eq!(result.scripts.len(), 1);
    assert_eq!(result.scripts[0].target_id, "page-1");
    assert_eq!(result.scripts[0].identifier, "theme-script-1");
    server.await.unwrap();
}

#[tokio::test]
async fn incompatible_utility_page_does_not_block_main_task_theme() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"main-task","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-task"}},{{"id":"utility","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/utility"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);

        for compatible in [true, false] {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let request = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["method"], "Runtime.evaluate");
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":{compatible}}}}}}}"#,
                        request["id"]
                    )
                    .into(),
                ))
                .await
                .unwrap();
        }

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..3 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            let result = match index {
                0 => {
                    assert_eq!(call["method"], "Page.enable");
                    "{}"
                }
                1 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    r#"{"identifier":"theme-script-main"}"#
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    let expression = call["params"]["expression"].as_str().unwrap();
                    assert!(expression.contains("data-codex-assistant-theme"));
                    assert!(expression.contains("getComputedStyle"));
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);

    let result = apply_theme_on_pages_for_version(&endpoint, "26.721.4979.0", &pack, &[], 1_000)
        .await
        .unwrap();

    assert_eq!(result.applied_pages, 1);
    assert_eq!(result.scripts.len(), 1);
    assert_eq!(result.scripts[0].target_id, "main-task");
    server.await.unwrap();
}

#[tokio::test]
async fn theme_is_not_applied_until_computed_style_is_verified() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"main-task","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-task"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let compatibility = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let compatibility: serde_json::Value = serde_json::from_str(&compatibility).unwrap();
        socket
            .send(Message::Text(
                format!(
                    r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                    compatibility["id"]
                )
                .into(),
            ))
            .await
            .unwrap();
        drop(socket);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..5 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            let result = match index {
                0 => {
                    assert_eq!(call["method"], "Page.enable");
                    "{}"
                }
                1 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    r#"{"identifier":"theme-script-main"}"#
                }
                2 => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    let expression = call["params"]["expression"].as_str().unwrap();
                    assert!(expression.contains("data-codex-assistant-theme"));
                    assert!(expression.contains("getComputedStyle"));
                    r#"{"result":{"type":"boolean","value":false}}"#
                }
                3 => {
                    assert_eq!(call["method"], "Page.removeScriptToEvaluateOnNewDocument");
                    assert_eq!(call["params"]["identifier"], "theme-script-main");
                    "{}"
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);

    let result =
        apply_theme_on_pages_for_version(&endpoint, "26.721.4979.0", &pack, &[], 1_000).await;

    assert!(matches!(result, Err(ThemeEngineError::PartialApplication)));
    server.await.unwrap();
}

#[tokio::test]
async fn two_compatible_primary_targets_fail_closed_before_injection() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"main-a","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-a"}},{{"id":"main-b","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-b"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let request = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["method"], "Runtime.evaluate");
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                        request["id"]
                    )
                    .into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);
    let result =
        apply_theme_on_pages_for_version(&endpoint, "26.721.4979.0", &pack, &[], 1_000).await;
    assert!(matches!(
        result,
        Err(ThemeEngineError::AmbiguousPrimaryTarget)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn unsupported_version_fails_before_any_target_discovery() {
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        9,
        r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}"#,
    )
    .unwrap();
    let pack = bundled_theme_packs().remove(0);
    let result = apply_theme_on_pages_for_version(&endpoint, "27.1.0.0", &pack, &[], 1_000).await;
    assert!(matches!(result, Err(ThemeEngineError::UnsupportedVersion)));
}
