use poko_core::note::Card;
use poko_core::parse_note;

const GAIN_PATH: &str = "learning/画像処理/カメラ/露出/ゲイン.md";

fn gain() -> String {
    include_str!("fixtures/vault/learning/画像処理/カメラ/露出/ゲイン.md").to_owned()
}

#[test]
fn gain_cards_match_question_answer_and_rubric() {
    let cards = parse_note(GAIN_PATH, &gain()).unwrap().cards();
    assert_eq!(cards.len(), 4);

    assert_eq!(cards[0].level, poko_core::Level::Beginner);
    assert_eq!(cards[0].question, "ゲインとは？");
    assert_eq!(
        cards[0].answer,
        "ゲインとは、撮像素子の出力信号を増幅する倍率である。"
    );
    assert_eq!(cards[0].rubric, None);
    assert!(cards[0].refs.is_empty());

    assert_eq!(cards[1].question, "なぜゲインを上げるとノイズが目立つ？");
    assert_eq!(
        cards[1].answer,
        "ゲインを上げるとノイズが目立つのは、受光量は増えず、信号とノイズをまとめて増幅するからである。"
    );
    assert_eq!(
        cards[1].rubric.as_deref(),
        Some("「受光量は増えない」「S/N 比は改善しない」に触れている")
    );

    assert_eq!(
        cards[2].question,
        "露光を延ばすのとゲインを上げるのは何が違う？"
    );
    assert_eq!(
        cards[2].answer,
        "露光を延ばすと受光量が増えて S/N 比が上がるが動体はぶれる。ゲインは増幅だけなので、ぶれは増えないがノイズが増える。"
    );
    assert_eq!(
        cards[2].rubric.as_deref(),
        Some("受光量/ぶれ/ノイズの 3 点の対比がある")
    );

    assert_eq!(cards[3].level, poko_core::Level::Advanced);
    assert_eq!(
        cards[3].question,
        "コンベア上のかんばんが暗くて QR が読めない。何をどの順で調整する？理由は？"
    );
    assert_eq!(
        cards[3].answer,
        "暗くて QR が読めないときは、照明 → 露光(ぶれが許す範囲) → ゲイン(最後)の順に調整する。"
    );
    assert_eq!(
        cards[3].rubric.as_deref(),
        Some("照明が先 / 露光はぶれの許容範囲まで / ゲインは最後でノイズ増に触れている")
    );
}

#[test]
fn stable_keys_are_fixed_for_the_same_question() {
    let cards = parse_note(GAIN_PATH, &gain()).unwrap().cards();
    assert_eq!(
        card_keys(&cards),
        vec![
            "gain/beginner/34a0dbf6827d4910",
            "gain/intermediate/d090164090665435",
            "gain/intermediate/ea21551758cb9821",
            "gain/advanced/e0d77ab91ff47abc",
        ]
    );
}

#[test]
fn reordering_items_does_not_change_other_keys() {
    let original = gain();
    let first = "\
- ゲインを上げるとノイズが目立つのは、受光量は増えず、信号とノイズをまとめて増幅するからである。
  - Q: なぜゲインを上げるとノイズが目立つ？
  - 採点: 「受光量は増えない」「S/N 比は改善しない」に触れている
";
    let second = "\
- 露光を延ばすと受光量が増えて S/N 比が上がるが動体はぶれる。ゲインは増幅だけなので、ぶれは増えないがノイズが増える。
  - Q: 露光を延ばすのとゲインを上げるのは何が違う？
  - 採点: 受光量/ぶれ/ノイズの 3 点の対比がある
";
    let swapped = original.replacen(&format!("{first}{second}"), &format!("{second}{first}"), 1);
    assert_ne!(original, swapped);

    let before = parse_note(GAIN_PATH, &original).unwrap().cards();
    let after = parse_note(GAIN_PATH, &swapped).unwrap().cards();
    assert_eq!(
        key_for(&before, "ゲインとは？"),
        key_for(&after, "ゲインとは？")
    );
    assert_eq!(
        key_for(&before, "なぜゲインを上げるとノイズが目立つ？"),
        key_for(&after, "なぜゲインを上げるとノイズが目立つ？")
    );
    assert_eq!(
        key_for(&before, "露光を延ばすのとゲインを上げるのは何が違う？"),
        key_for(&after, "露光を延ばすのとゲインを上げるのは何が違う？")
    );
}

#[test]
fn question_inside_code_block_is_not_a_card() {
    let markdown = "\
---
id: fence
---
# フェンス

## 初級
- 本物の説明である。
  - Q: 本物の質問？
- コードだけが質問に見える説明である。
  ```
  - Q: コードの中
  ```
- コードと本物の質問がある説明である。
  ```
  - Q: これもコード
  ```
  - Q: コードの外
";
    let cards = parse_note("learning/画像処理/カメラ/露出/フェンス.md", markdown)
        .unwrap()
        .cards();
    let questions: Vec<_> = cards.iter().map(|card| card.question.as_str()).collect();
    assert_eq!(questions, vec!["本物の質問？", "コードの外"]);
}

#[test]
fn integration_card_keeps_refs() {
    let markdown = "\
---
id: gain
---
# ゲイン

## 初級
- 土台である。
  - Q: 土台は？

## 統合
- 露光とゲインを比べる説明である。
  - Q: どちらを先に変える？
  - 参照: 露光、シャッター
";
    let cards = parse_note(GAIN_PATH, markdown).unwrap().cards();
    assert!(cards[0].refs.is_empty());
    assert_eq!(cards[1].level, poko_core::Level::Integration);
    assert_eq!(
        cards[1].refs,
        vec!["露光".to_owned(), "シャッター".to_owned()]
    );
}

fn card_keys(cards: &[Card]) -> Vec<&str> {
    cards.iter().map(|card| card.stable_key.as_str()).collect()
}

fn key_for(cards: &[Card], question: &str) -> String {
    cards
        .iter()
        .find(|card| card.question == question)
        .unwrap()
        .stable_key
        .clone()
}
