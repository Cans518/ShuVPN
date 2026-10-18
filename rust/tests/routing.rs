use shuvpn_core::routing::{
    compiler,
    engine::{self, Evaluation},
    model::*,
};
use std::collections::BTreeMap;

fn configuration(rules: Vec<&str>) -> Profile {
    Profile {
        name: "test".to_owned(),
        rules: rules
            .into_iter()
            .map(|rule| rule.to_owned().into())
            .collect(),
        automatic_campus: true,
        groups: vec![],
        providers: BTreeMap::new(),
    }
}

fn flow(host: &str) -> Flow {
    Flow {
        host: host.to_owned(),
        ips: vec![],
        port: 443,
        network: Network::Tcp,
        inbound: Inbound::Http,
        process: None,
        domain_candidates: Vec::new(),
    }
}

#[test]
fn ambiguous_dns_names_cannot_bypass_domain_rejection_but_irrelevant_aliases_do_not_block_ip_rules(
) -> anyhow::Result<()> {
    let mut input = flow("198.51.100.7");
    input.domain_candidates = vec!["ads.example".into(), "content.example".into()];
    assert_eq!(
        decision(
            configuration(vec!["DOMAIN,ads.example,REJECT", "MATCH,DIRECT"]),
            Overrides::default(),
            input.clone()
        )
        .err()
        .and_then(|error| error.downcast::<RoutingFailure>().ok())
        .map(|error| error.code),
        Some("dns_ambiguous".into())
    );
    assert_eq!(
        decision(
            configuration(vec![
                "IP-CIDR,198.51.100.0/24,DIRECT,no-resolve",
                "MATCH,REJECT"
            ]),
            Overrides::default(),
            input.clone()
        )?
        .action,
        Action::Direct
    );
    assert_eq!(
        decision(
            configuration(vec!["DOMAIN-SUFFIX,example,DIRECT", "MATCH,REJECT"]),
            Overrides::default(),
            input
        )?
        .action,
        Action::Direct
    );
    Ok(())
}

#[test]
fn process_rules_match_supplied_metadata_but_missing_identity_never_falls_through(
) -> anyhow::Result<()> {
    let path = if cfg!(windows) {
        r"C:\Apps\fixture.exe"
    } else {
        "/opt/apps/fixture"
    };
    let name = if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    };
    for rule in [
        format!("PROCESS-NAME,{name},REJECT"),
        format!("PROCESS-PATH,{path},REJECT"),
    ] {
        let profile = configuration(vec![&rule, "MATCH,DIRECT"]);
        let compiled = compiler::compile(&profile, &Overrides::default(), &BTreeMap::new())?;
        assert!(compiled.uses_process);
        assert_eq!(
            engine::evaluate(Some(&compiled), &[], 1, &flow("target.example"))
                .err()
                .map(|error| error.code),
            Some("process".into())
        );
        for inbound in [Inbound::Http, Inbound::Socks, Inbound::Tun] {
            for network in [Network::Tcp, Network::Udp] {
                let mut input = flow("target.example");
                input.inbound = inbound;
                input.network = network;
                input.process = Some(ProcessIdentity {
                    pid: 0,
                    name: name.into(),
                    path: path.into(),
                    source: "preview".into(),
                    confidence: ProcessConfidence::Preview,
                });
                assert!(matches!(
                    engine::evaluate(Some(&compiled), &[], 1, &input)?,
                    Evaluation::Decided(RouteDecision {
                        action: Action::Reject,
                        ..
                    })
                ));
                input
                    .process
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("process missing"))?
                    .name = "different".into();
                input
                    .process
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("process missing"))?
                    .path = "/different".into();
                assert!(matches!(
                    engine::evaluate(Some(&compiled), &[], 1, &input)?,
                    Evaluation::Decided(RouteDecision {
                        action: Action::Direct,
                        ..
                    })
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_process_logic_is_not_inverted_and_known_branches_can_short_circuit() -> anyhow::Result<()>
{
    for rule in [
        "PROCESS-NAME,fixture,REJECT",
        "NOT,((PROCESS-NAME,fixture)),REJECT",
        "AND,((PROCESS-NAME,fixture),(NETWORK,tcp)),REJECT",
    ] {
        assert!(decision(
            configuration(vec![rule, "MATCH,DIRECT"]),
            Overrides::default(),
            flow("target.example")
        )
        .is_err());
    }
    for (rule, expected) in [
        (
            "AND,((PROCESS-NAME,fixture),(NETWORK,udp)),REJECT",
            Action::Direct,
        ),
        (
            "OR,((PROCESS-NAME,fixture),(NETWORK,tcp)),REJECT",
            Action::Reject,
        ),
    ] {
        assert_eq!(
            decision(
                configuration(vec![rule, "MATCH,DIRECT"]),
                Overrides::default(),
                flow("target.example")
            )?
            .action,
            expected
        );
    }
    let profile: Profile = compiler::parse_document(
        r#"{"rules":[{"rule":"PROCESS-NAME,fixture,REJECT","enabled":false},"MATCH,DIRECT"]}"#,
    )?;
    let compiled = compiler::compile(&profile, &Overrides::default(), &BTreeMap::new())?;
    assert!(!compiled.uses_process);
    Ok(())
}

#[test]
fn invalid_process_rules_and_control_characters_are_rejected() -> anyhow::Result<()> {
    for rule in [
        "PROCESS-NAME,,REJECT",
        "PROCESS-NAME,/opt/app,REJECT",
        "PROCESS-PATH,relative/app,REJECT",
        "PROCESS-NAME,app,REJECT,no-resolve",
        r#"PROCESS-NAME,"invalid\nname",REJECT"#,
    ] {
        assert!(
            compiler::compile(
                &configuration(vec![rule, "MATCH,DIRECT"]),
                &Overrides::default(),
                &BTreeMap::new()
            )
            .is_err(),
            "{rule}"
        );
    }
    let mut input = flow("target.example");
    input.process = Some(ProcessIdentity {
        pid: 0,
        name: "invalid\nname".into(),
        path: String::new(),
        source: "preview".into(),
        confidence: ProcessConfidence::Preview,
    });
    assert!(decision(
        configuration(vec!["MATCH,DIRECT"]),
        Overrides::default(),
        input
    )
    .is_err());
    Ok(())
}

#[test]
fn process_unknown_and_dns_unknown_remain_separate_and_classical_sets_keep_identity(
) -> anyhow::Result<()> {
    let compiled = compiler::compile(
        &configuration(vec![
            "AND,((PROCESS-NAME,fixture),(IP-CIDR,10.0.0.0/8)),REJECT",
            "MATCH,DIRECT",
        ]),
        &Overrides::default(),
        &BTreeMap::new(),
    )?;
    let mut input = flow("target.example");
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &input)?,
        Evaluation::NeedAddress
    ));
    input.ips = vec!["127.0.0.1".parse()?];
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &input)?,
        Evaluation::Decided(RouteDecision {
            action: Action::Direct,
            ..
        })
    ));
    input.ips = vec!["10.0.0.1".parse()?];
    assert_eq!(
        engine::evaluate(Some(&compiled), &[], 1, &input)
            .err()
            .map(|error| error.code),
        Some("process".into())
    );
    let mut profile = configuration(vec!["RULE-SET,p,REJECT", "MATCH,DIRECT"]);
    profile.providers.insert(
        "p".into(),
        Provider {
            kind: "inline".into(),
            behavior: "classical".into(),
            format: "yaml".into(),
            payload: vec![],
            path: None,
            url: None,
            interval: None,
            data: None,
        },
    );
    let compiled = compiler::compile(
        &profile,
        &Overrides::default(),
        &BTreeMap::from([("p".into(), vec!["PROCESS-NAME,fixture".into()])]),
    )?;
    assert!(compiled.uses_process);
    assert_eq!(
        engine::evaluate(Some(&compiled), &[], 1, &input)
            .err()
            .map(|error| error.code),
        Some("process".into())
    );
    input.process = Some(ProcessIdentity {
        pid: 0,
        name: "fixture".into(),
        path: String::new(),
        source: "preview".into(),
        confidence: ProcessConfidence::Preview,
    });
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &input)?,
        Evaluation::Decided(RouteDecision {
            action: Action::Reject,
            ..
        })
    ));
    Ok(())
}

#[test]
fn process_case_and_native_path_separators_follow_the_platform_without_changing_regex_literals(
) -> anyhow::Result<()> {
    let (rule, path, expected) = if cfg!(windows) {
        (
            "PROCESS-PATH,c:/APPS/FIXTURE.exe,REJECT",
            r"C:\Apps\fixture.exe",
            Action::Reject,
        )
    } else {
        (
            "PROCESS-PATH,/opt/apps/fixture,REJECT",
            "/opt/apps/Fixture",
            Action::Direct,
        )
    };
    let mut input = flow("target.example");
    input.process = Some(ProcessIdentity {
        pid: 0,
        name: String::new(),
        path: path.into(),
        source: "preview".into(),
        confidence: ProcessConfidence::Preview,
    });
    assert_eq!(
        decision(
            configuration(vec![rule, "MATCH,DIRECT"]),
            Overrides::default(),
            input
        )?
        .action,
        expected
    );
    let compiled = compiler::compile(
        &configuration(vec![
            r#"AND,((DOMAIN-REGEX,(PROCESS-NAME,"a)b"),(NETWORK,tcp)),REJECT"#,
            "MATCH,DIRECT",
        ]),
        &Overrides::default(),
        &BTreeMap::new(),
    )?;
    assert!(!compiled.uses_process);
    Ok(())
}

#[test]
fn literal_process_values_can_quote_commas_and_unbalanced_expression_characters(
) -> anyhow::Result<()> {
    let name = "fixture[1,case.exe";
    let path = if cfg!(windows) {
        r"C:\Apps\fixture[1,case.exe"
    } else {
        "/opt/apps/fixture[1,case.exe"
    };
    let quoted_name = serde_json::to_string(name)?;
    let quoted_path = serde_json::to_string(path)?;
    for rule in [
        format!("PROCESS-NAME,{quoted_name},REJECT"),
        format!("PROCESS-PATH,{quoted_path},REJECT"),
        format!("AND,((PROCESS-NAME,{quoted_name}),(NETWORK,tcp)),REJECT"),
    ] {
        let mut input = flow("target.example");
        input.process = Some(ProcessIdentity {
            pid: 0,
            name: name.into(),
            path: path.into(),
            source: "preview".into(),
            confidence: ProcessConfidence::Preview,
        });
        assert_eq!(
            decision(
                configuration(vec![&rule, "MATCH,DIRECT"]),
                Overrides::default(),
                input
            )?
            .action,
            Action::Reject
        );
    }
    Ok(())
}

fn decision(profile: Profile, edits: Overrides, flow: Flow) -> anyhow::Result<RouteDecision> {
    let compiled = compiler::compile(&profile, &edits, &BTreeMap::new())?;
    match engine::evaluate(Some(&compiled), &[], 7, &flow)? {
        Evaluation::Decided(decision) => Ok(decision),
        Evaluation::NeedAddress => Err(anyhow::anyhow!("unexpected DNS requirement")),
    }
}

#[test]
fn domain_rules_normalize_case_idna_and_suffix_boundaries() -> anyhow::Result<()> {
    for (host, expected) in [
        ("SHU.edu.cn.", Action::ShuVpn),
        ("oa.shu.edu.cn", Action::ShuVpn),
        ("evilshu.edu.cn", Action::Direct),
        ("oa.shu.edu.cn.attacker.invalid", Action::Direct),
    ] {
        assert_eq!(
            decision(
                configuration(vec!["DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN", "MATCH,DIRECT"]),
                Overrides::default(),
                flow(host)
            )?
            .action,
            expected
        );
    }
    assert_eq!(
        decision(
            configuration(vec!["DOMAIN,bücher.example,REJECT", "MATCH,DIRECT"]),
            Overrides::default(),
            flow("XN--BCHER-KVA.example")
        )?
        .action,
        Action::Reject
    );
    assert_eq!(
        decision(
            configuration(vec!["DOMAIN-KEYWORD,ads,REJECT", "MATCH,DIRECT"]),
            Overrides::default(),
            flow("ads.example")
        )?
        .action,
        Action::Reject
    );
    Ok(())
}

#[test]
fn first_match_prevents_falling_through_after_a_shu_vpn_decision() -> anyhow::Result<()> {
    let selected = decision(
        configuration(vec![
            "DOMAIN,target.example,SHU-VPN",
            "DOMAIN,target.example,DIRECT",
            "MATCH,REJECT",
        ]),
        Overrides::default(),
        flow("target.example"),
    )?;
    assert_eq!(selected.action, Action::ShuVpn);
    assert_eq!(selected.rule_id, "profile:0");
    assert_eq!(selected.version, 7);
    Ok(())
}

#[test]
fn nested_logic_regex_and_inbound_share_the_normalized_flow() -> anyhow::Result<()> {
    let rule = r"AND,((DOMAIN-REGEX,^api[0-9]{1,3}\.example$),(OR,((DST-PORT,443),(DST-PORT,8443))),(NOT,((NETWORK,udp)))),REJECT";
    for inbound in [Inbound::Http, Inbound::Socks, Inbound::Tun] {
        let mut candidate = flow("API123.EXAMPLE.");
        candidate.inbound = inbound;
        assert_eq!(
            decision(
                configuration(vec![rule, "MATCH,DIRECT"]),
                Overrides::default(),
                candidate
            )?
            .action,
            Action::Reject
        );
    }
    for host in [
        "api.example",
        "api1234.example",
        "api123.example.attacker",
        "10.0.0.1",
    ] {
        assert_eq!(
            decision(
                configuration(vec![rule, "MATCH,DIRECT"]),
                Overrides::default(),
                flow(host)
            )?
            .action,
            Action::Direct
        );
    }
    for rule in [
        r"DOMAIN-REGEX,^api[.(){}]example$,REJECT",
        r"DOMAIN-REGEX,^[a,b]{1,3}\.example$,REJECT",
    ] {
        let candidate = if rule.contains("api") {
            "api.example"
        } else {
            "aba.example"
        };
        assert_eq!(
            decision(
                configuration(vec![rule, "MATCH,DIRECT"]),
                Overrides::default(),
                flow(candidate)
            )?
            .action,
            Action::Reject
        );
    }
    assert_eq!(
        decision(
            configuration(vec!["DOMAIN-REGEX,.*,REJECT", "MATCH,DIRECT"]),
            Overrides::default(),
            flow("127.0.0.1")
        )?
        .action,
        Action::Direct
    );
    for rule in [
        r"DOMAIN-REGEX,^[]a][.]example$,REJECT",
        r"AND,((DOMAIN-REGEX,^[]a][.]example$),(NETWORK,tcp)),REJECT",
        r"DOMAIN-REGEX,^[^]b][.]example$,REJECT",
    ] {
        assert_eq!(
            decision(
                configuration(vec![rule, "MATCH,DIRECT"]),
                Overrides::default(),
                flow("a.example")
            )?
            .action,
            Action::Reject
        );
    }
    Ok(())
}

#[test]
fn logic_keeps_unresolved_ip_unknown_but_uses_definite_boolean_results() -> anyhow::Result<()> {
    for (rule, pending, action) in [
        (
            "AND,((IP-CIDR,10.0.0.0/8),(DOMAIN,other.example)),REJECT",
            false,
            Action::Direct,
        ),
        (
            "OR,((IP-CIDR,10.0.0.0/8),(DOMAIN,target.example)),REJECT",
            false,
            Action::Reject,
        ),
        ("NOT,((IP-CIDR,10.0.0.0/8)),REJECT", true, Action::Direct),
        (
            "AND,((DOMAIN,target.example),(IP-CIDR,10.0.0.0/8)),REJECT",
            true,
            Action::Direct,
        ),
        (
            "NOT,((IP-CIDR,10.0.0.0/8,no-resolve)),REJECT",
            false,
            Action::Reject,
        ),
    ] {
        let compiled = compiler::compile(
            &configuration(vec![rule, "MATCH,DIRECT"]),
            &Overrides::default(),
            &BTreeMap::new(),
        )?;
        match engine::evaluate(Some(&compiled), &[], 9, &flow("target.example"))? {
            Evaluation::NeedAddress => assert!(pending, "{rule}"),
            Evaluation::Decided(selected) => {
                assert!(!pending, "{rule}");
                assert_eq!(selected.action, action, "{rule}");
            }
        }
        let mut resolved = flow("target.example");
        resolved.ips = vec!["10.1.2.3".parse()?];
        let selected = engine::evaluate(Some(&compiled), &[], 9, &resolved)?;
        assert!(matches!(selected, Evaluation::Decided(_)), "{rule}");
    }
    Ok(())
}

#[test]
fn logical_classical_sets_and_nested_ruleset_references_keep_no_resolve() -> anyhow::Result<()> {
    let mut profile = configuration(vec![
        "AND,((RULE-SET,p,no-resolve),(NETWORK,tcp)),REJECT",
        "MATCH,DIRECT",
    ]);
    profile.providers.insert(
        "p".to_owned(),
        Provider {
            kind: "inline".to_owned(),
            behavior: "classical".to_owned(),
            format: "yaml".to_owned(),
            payload: vec![],
            path: None,
            url: None,
            interval: None,
            data: None,
        },
    );
    let payloads = BTreeMap::from([(
        "p".to_owned(),
        vec!["AND,((DOMAIN-REGEX,^target[.]example$),(IP-CIDR,10.0.0.0/8))".to_owned()],
    )]);
    let compiled = compiler::compile(&profile, &Overrides::default(), &payloads)?;
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &flow("target.example"))?,
        Evaluation::Decided(RouteDecision {
            action: Action::Direct,
            ..
        })
    ));
    let mut resolved = flow("target.example");
    resolved.ips = vec!["10.1.2.3".parse()?];
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &resolved)?,
        Evaluation::Decided(RouteDecision {
            action: Action::Reject,
            ..
        })
    ));
    let recursive = BTreeMap::from([(
        "p".to_owned(),
        vec!["AND,((RULE-SET,p),(NETWORK,tcp))".to_owned()],
    )]);
    assert!(compiler::compile(&profile, &Overrides::default(), &recursive).is_err());
    Ok(())
}

#[test]
fn expression_and_regex_invalid_inputs_and_budgets_are_rejected() -> anyhow::Result<()> {
    for rule in [
        "AND,(),REJECT",
        "OR,((DOMAIN,a.example),),REJECT",
        "NOT,((DOMAIN,a.example),(DOMAIN,b.example)),REJECT",
        "AND,((MATCH,DIRECT),(NETWORK,tcp)),REJECT",
        "NOT,((DOMAIN,a.example,REJECT)),DIRECT",
        "AND,((DOMAIN,a.example)),REJECT,no-resolve",
        "AND,((DOMAIN,a.example)),REJECT,ignored",
        "DOMAIN-REGEX,(?=a),REJECT",
        r"DOMAIN-REGEX,(a)\1,REJECT",
        "DOMAIN-REGEX,[abc,REJECT",
    ] {
        assert!(
            compiler::compile(
                &configuration(vec![rule, "MATCH,DIRECT"]),
                &Overrides::default(),
                &BTreeMap::new()
            )
            .is_err(),
            "{rule}"
        );
    }
    let mut expression = "DOMAIN,target.example".to_owned();
    for _ in 0..17 {
        expression = format!("NOT,(({expression}))");
    }
    let deep = format!("{expression},REJECT");
    assert!(compiler::compile(
        &configuration(vec![&deep, "MATCH,DIRECT"]),
        &Overrides::default(),
        &BTreeMap::new()
    )
    .is_err());
    let oversized = format!("DOMAIN-REGEX,{},REJECT", "a".repeat(1025));
    assert!(compiler::compile(
        &configuration(vec![&oversized, "MATCH,DIRECT"]),
        &Overrides::default(),
        &BTreeMap::new()
    )
    .is_err());
    let mut profile = configuration(vec!["MATCH,DIRECT"]);
    profile.rules.splice(
        0..0,
        (0..129).map(|index| format!("DOMAIN-REGEX,^host{index}[.]example$,REJECT").into()),
    );
    assert!(compiler::compile(&profile, &Overrides::default(), &BTreeMap::new()).is_err());
    assert!(compiler::compile(
        &configuration(vec!["DOMAIN-REGEX,a{100000},REJECT", "MATCH,DIRECT"]),
        &Overrides::default(),
        &BTreeMap::new()
    )
    .is_err());
    Ok(())
}

#[test]
fn ip_rules_and_no_resolve_work_for_both_families() -> anyhow::Result<()> {
    let profile = configuration(vec![
        "IP-CIDR,10.0.0.0/8,SHU-VPN,no-resolve",
        "IP-CIDR6,2001:db8::/32,REJECT,no-resolve",
        "MATCH,DIRECT",
    ]);
    assert_eq!(
        decision(
            profile.clone(),
            Overrides::default(),
            flow("unknown.example")
        )?
        .action,
        Action::Direct
    );
    assert_eq!(
        decision(profile.clone(), Overrides::default(), flow("10.1.2.3"))?.action,
        Action::ShuVpn
    );
    assert_eq!(
        decision(profile, Overrides::default(), flow("2001:db8::4"))?.action,
        Action::Reject
    );
    let compiled = compiler::compile(
        &configuration(vec!["IP-CIDR,10.0.0.0/8,SHU-VPN", "MATCH,DIRECT"]),
        &Overrides::default(),
        &BTreeMap::new(),
    )?;
    assert!(matches!(
        engine::evaluate(Some(&compiled), &[], 1, &flow("unknown.example"))?,
        Evaluation::NeedAddress
    ));
    Ok(())
}

#[test]
fn ports_network_and_inbound_types_have_defined_boundaries() -> anyhow::Result<()> {
    for (port, expected) in [
        (80, Action::Reject),
        (443, Action::Reject),
        (8080, Action::Reject),
        (8090, Action::Reject),
        (8091, Action::Direct),
    ] {
        let mut input = flow("example.com");
        input.port = port;
        assert_eq!(
            decision(
                configuration(vec!["DST-PORT,80/443/8080-8090,REJECT", "MATCH,DIRECT"]),
                Overrides::default(),
                input
            )?
            .action,
            expected
        );
    }
    let mut input = flow("example.com");
    input.network = Network::Udp;
    assert_eq!(
        decision(
            configuration(vec!["NETWORK,udp,REJECT", "MATCH,DIRECT"]),
            Overrides::default(),
            input
        )?
        .action,
        Action::Reject
    );
    for inbound in [Inbound::Http, Inbound::Socks, Inbound::Tun] {
        let mut input = flow("example.com");
        input.inbound = inbound;
        let expected = if inbound == Inbound::Tun {
            Action::Direct
        } else {
            Action::Reject
        };
        assert_eq!(
            decision(
                configuration(vec!["IN-TYPE,HTTP/SOCKS,REJECT", "MATCH,DIRECT"]),
                Overrides::default(),
                input
            )?
            .action,
            expected
        );
    }
    Ok(())
}

#[test]
fn manual_groups_resolve_only_the_selected_known_outbound() -> anyhow::Result<()> {
    let mut profile = configuration(vec!["MATCH,choice"]);
    profile.groups.push(PolicyGroup {
        name: "choice".to_owned(),
        kind: "select".to_owned(),
        proxies: vec!["DIRECT".to_owned(), "SHU-VPN".to_owned()],
    });
    let mut edits = Overrides::default();
    edits
        .selections
        .insert("choice".to_owned(), "SHU-VPN".to_owned());
    assert_eq!(
        decision(profile.clone(), edits.clone(), flow("example.com"))?.action,
        Action::ShuVpn
    );
    edits
        .selections
        .insert("choice".to_owned(), "missing".to_owned());
    assert!(compiler::compile(&profile, &edits, &BTreeMap::new()).is_err());
    assert!(compiler::compile(
        &configuration(vec!["MATCH,unknown"]),
        &Overrides::default(),
        &BTreeMap::new()
    )
    .is_err());
    Ok(())
}

#[test]
fn overrides_profile_campus_append_and_final_have_deterministic_priority() -> anyhow::Result<()> {
    let profile = configuration(vec!["DOMAIN,profile.example,REJECT", "MATCH,DIRECT"]);
    let edits = Overrides {
        prepend: vec!["DOMAIN,first.example,DIRECT".to_owned().into()],
        append: vec!["DOMAIN-SUFFIX,example,REJECT".to_owned().into()],
        selections: BTreeMap::new(),
    };
    let route = sangfor_core::route::Route {
        host: "*.example".to_owned(),
        protocol: "tcp".to_owned(),
        port_min: 1,
        port_max: 65535,
        app_id: "campus".to_owned(),
        node_group_id: "group".to_owned(),
        addr_pretend: true,
        enable_tcp_pref_l3: false,
    };
    let compiled = compiler::compile(&profile, &edits, &BTreeMap::new())?;
    for (host, action, source) in [
        ("first.example", Action::Direct, "prepend"),
        ("profile.example", Action::Reject, "profile"),
        ("auto.example", Action::ShuVpn, "campus"),
        ("else.invalid", Action::Direct, "profile"),
    ] {
        let Evaluation::Decided(selected) = engine::evaluate(
            Some(&compiled),
            std::slice::from_ref(&route),
            1,
            &flow(host),
        )?
        else {
            return Err(anyhow::anyhow!("unexpected DNS"));
        };
        assert_eq!(selected.action, action);
        assert_eq!(selected.source, source);
    }
    Ok(())
}

#[test]
fn final_match_is_normalized_and_duplicate_or_unknown_rules_are_rejected() -> anyhow::Result<()> {
    let compiled = compiler::compile(
        &configuration(vec!["MATCH,DIRECT", "DOMAIN,blocked.example,REJECT"]),
        &Overrides::default(),
        &BTreeMap::new(),
    )?;
    assert_eq!(compiled.notices.len(), 1);
    assert_eq!(
        decision(
            configuration(vec!["MATCH,DIRECT", "DOMAIN,blocked.example,REJECT"]),
            Overrides::default(),
            flow("blocked.example")
        )?
        .action,
        Action::Reject
    );
    for rules in [
        vec!["MATCH,DIRECT", "MATCH,REJECT"],
        vec!["GEOIP,CN,DIRECT", "MATCH,DIRECT"],
        vec!["IP-CIDR,10.0.0.0/99,DIRECT", "MATCH,DIRECT"],
        vec!["DST-PORT,443-80,DIRECT", "MATCH,DIRECT"],
        vec!["RULE-SET,missing,REJECT", "MATCH,DIRECT"],
        vec!["MATCH,unused,DIRECT"],
    ] {
        assert!(compiler::compile(
            &configuration(rules),
            &Overrides::default(),
            &BTreeMap::new()
        )
        .is_err());
    }
    assert!(compiler::parse_document::<Profile>("rules: [MATCH,DIRECT]\nproxies: []").is_err());
    Ok(())
}

#[test]
fn expanded_rules_and_port_segments_are_bounded() {
    let ports = vec!["443"; 129].join("/");
    assert!(compiler::compile(
        &configuration(vec![&format!("DST-PORT,{ports},DIRECT"), "MATCH,DIRECT"]),
        &Overrides::default(),
        &BTreeMap::new(),
    )
    .is_err());
    let mut profile = configuration(vec!["MATCH,DIRECT"]);
    profile
        .rules
        .splice(0..0, vec!["RULE-SET,p,REJECT".to_owned().into(); 6]);
    profile.providers.insert(
        "p".to_owned(),
        Provider {
            kind: "inline".to_owned(),
            behavior: "domain".to_owned(),
            format: "yaml".to_owned(),
            payload: vec![],
            path: None,
            url: None,
            interval: None,
            data: None,
        },
    );
    let payloads = BTreeMap::from([("p".to_owned(), vec!["example.com".to_owned(); 20_000])]);
    assert!(compiler::compile(&profile, &Overrides::default(), &payloads).is_err());
}

#[test]
fn rule_sets_keep_domain_wildcards_and_classical_conditions_separate_from_policy(
) -> anyhow::Result<()> {
    for (behavior, payload, host, expected) in [
        (
            "domain",
            vec!["+.example.com"],
            "example.com",
            Action::Reject,
        ),
        (
            "domain",
            vec![".example.com"],
            "example.com",
            Action::Direct,
        ),
        (
            "domain",
            vec!["*.*.example.com"],
            "a.b.example.com",
            Action::Reject,
        ),
        (
            "domain",
            vec!["*.example.com"],
            "a.b.example.com",
            Action::Direct,
        ),
        (
            "domain",
            vec!["*.bücher.example."],
            "a.xn--bcher-kva.example",
            Action::Reject,
        ),
        ("ipcidr", vec!["10.0.0.0/8"], "10.1.2.3", Action::Reject),
        (
            "classical",
            vec!["DOMAIN-SUFFIX,example.com"],
            "a.example.com",
            Action::Reject,
        ),
    ] {
        let mut profile = configuration(vec!["RULE-SET,test,REJECT,no-resolve", "MATCH,DIRECT"]);
        profile.providers.insert(
            "test".to_owned(),
            Provider {
                kind: "inline".to_owned(),
                behavior: behavior.to_owned(),
                format: "yaml".to_owned(),
                payload: vec![],
                path: None,
                url: None,
                interval: None,
                data: None,
            },
        );
        let payloads = BTreeMap::from([(
            "test".to_owned(),
            payload.into_iter().map(str::to_owned).collect(),
        )]);
        let compiled = compiler::compile(&profile, &Overrides::default(), &payloads)?;
        let Evaluation::Decided(selected) = engine::evaluate(Some(&compiled), &[], 1, &flow(host))?
        else {
            return Err(anyhow::anyhow!("no-resolve incorrectly triggered DNS"));
        };
        assert_eq!(selected.action, expected);
    }
    Ok(())
}
