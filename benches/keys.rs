use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mkey_exporter::config::{Rule, RuleGroup, RulePattern};
use mkey_exporter::keys::LabelParser;
use mtop_client::Meta;

fn new_metas() -> Vec<Meta> {
    vec![
        Meta {
            key: "EP:3255:lstate.h".to_owned(),
            ..Default::default()
        },
        Meta {
            key: "EP:3262:example.c".to_owned(),
            ..Default::default()
        },
        Meta {
            key: "EP:3290:base64.h".to_owned(),
            ..Default::default()
        },
        Meta {
            key: "EP:3292:darwin_priv.c".to_owned(),
            ..Default::default()
        },
        Meta {
            key: "EP:3325:slab_automove.c".to_owned(),
            ..Default::default()
        },
    ]
}

fn new_config() -> RuleGroup {
    RuleGroup {
        name: "bench".to_owned(),
        rules: vec![
            Rule {
                pattern: RulePattern::new(r"^\w+:([\w\-]+):").unwrap(),
                label_name: "user".to_owned(),
                label_value: "$1".to_owned(),
            },
            Rule {
                pattern: RulePattern::new(r"^(\w+):").unwrap(),
                label_name: "type".to_owned(),
                label_value: "$1".to_owned(),
            },
        ],
    }
}

fn keys_benchmark(c: &mut Criterion) {
    let cfg = new_config();
    let parser = LabelParser::new(&cfg);
    let metas = new_metas();

    c.bench_function("LabelParser::extract()", |b| {
        b.iter(|| {
            for m in metas.iter() {
                let _ = parser.extract(black_box(m));
            }
        })
    });
}

criterion_group!(benches, keys_benchmark);
criterion_main!(benches);
