from nexdir_notify.text import ZWSP, first_paragraph, highlights, parse_version, plain, truncate


def test_plain_escapes_markdown_mentions_and_newlines():
    text = plain("@everyone **free** [nitro](https://evil.example) <@123>\nnext line", 200)
    assert "@everyone" not in text
    assert "@" + ZWSP + "everyone" in text
    assert "\\*\\*free\\*\\*" in text
    assert "\\[nitro\\]" in text
    assert "\\<@" in text
    assert "\n" not in text


def test_plain_strips_control_and_bidi_characters():
    assert plain("a‮b\x07c", 50) == "abc"


def test_truncate_adds_ellipsis_and_never_ends_on_escape():
    assert truncate("abcdef", 10) == "abcdef"
    assert truncate("abcdef", 4) == "abc…"
    assert truncate("ab\\*cd", 4) == "ab…"


def test_parse_version():
    assert parse_version("v1.2.0") == ("1.2.0", False)
    assert parse_version("1.2.0") == ("1.2.0", False)
    assert parse_version("v2.0.0-beta.1") == ("2.0.0-beta.1", True)
    assert parse_version("v1.2.0+build.5") == ("1.2.0+build.5", False)
    for bad in ("latest", "v1.2", "release-1.0.0", "v1.2.0x", "v1.2.x"):
        assert parse_version(bad) is None


def test_highlights_takes_top_level_bullets_only():
    body = "<!-- hidden\n- not this -->\n## Added\n- One ![shot](a.png)\n  - nested\n* Two @here\n\nText\n+ Three"
    items, more = highlights(body)
    assert items == ["One", "Two @" + ZWSP + "here", "Three"]
    assert more == 0


def test_highlights_limit_reports_the_rest():
    body = "\n".join(f"- item {i}" for i in range(12))
    items, more = highlights(body, max_items=8)
    assert len(items) == 8 and more == 4


def test_first_paragraph_skips_headings():
    assert first_paragraph("# Title\n\nThis release fixes bugs.\n\nMore.") == "This release fixes bugs."
    assert first_paragraph(None) == ""
