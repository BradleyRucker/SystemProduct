#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Requirements extractor sidecar for SystemProduct.

Protocol:
  stdin:  one JSON line with either:
            { "blocks": [...], "doc_type": "..." }
          or
            { "sentences": [...], "doc_type": "..." }
  stdout: one JSON line:
            { "results": [...], "spacy_available": bool, "doc_type": "..." }
  stderr: log messages only

This parser is tuned for requirement-like statements and includes:
  - modal requirements ("shall/must/will")
  - implicit constraints in structured mode/parameter lines
  - component/subsystem-aware classification
  - conservative requirement-ID detection (prefix-based only)
"""

from __future__ import annotations

import hashlib
import json
import logging
import re
import sys
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple


# Force UTF-8 on Windows consoles.
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stdin, "reconfigure"):
    sys.stdin.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

logging.basicConfig(
    stream=sys.stderr,
    level=logging.INFO,
    format="[req_parser] %(levelname)s %(message)s",
)


# Optional spaCy (best-effort).
try:
    import spacy

    _NLP = spacy.load("en_core_web_sm")
    SPACY_OK = True
    logging.info("spaCy loaded OK")
except Exception as exc:
    _NLP = None
    SPACY_OK = False
    logging.warning("spaCy unavailable, rule mode only: %s", exc)


@dataclass
class ExtractConfig:
    use_spacy: bool = False
    enable_non_modal_constraints: bool = True
    enable_atomic_split: bool = False
    min_accept_score: float = 3.0
    min_non_modal_score: float = 3.5
    treat_will_as_normative: bool = False
    id_prefixes: Tuple[str, ...] = ("REQ", "SR", "SWR", "SYS", "SS", "SRS", "FR", "PR", "APSDS")


CFG = ExtractConfig()


# Core patterns.
SHALL_RE = re.compile(r"\b(shall|must|will)\b", re.IGNORECASE)
MODAL_PATTERNS: List[Tuple[str, re.Pattern[str]]] = [
    ("shall_not", re.compile(r"\bshall\s+not\b", re.IGNORECASE)),
    ("must_not", re.compile(r"\bmust\s+not\b", re.IGNORECASE)),
    ("may_not", re.compile(r"\bmay\s+not\b", re.IGNORECASE)),
    ("shall", re.compile(r"\bshall\b", re.IGNORECASE)),
    ("must", re.compile(r"\bmust\b", re.IGNORECASE)),
    ("required", re.compile(r"\b(is|required)\s+to\b|\bis\s+required\s+to\b", re.IGNORECASE)),
    ("should", re.compile(r"\bshould\b", re.IGNORECASE)),
    ("will", re.compile(r"\bwill\b", re.IGNORECASE)),
]

REQ_ID_PREFIX_RE = re.compile(
    r"\b(?P<prefix>REQ|SR|SWR|SYS|SS|SRS|FR|PR|APSDS)"
    r"(?P<sep>[-_ ]?)"
    r"(?P<num>\d{1,6}(?:\.\d{1,6})*)\b",
    re.IGNORECASE,
)

REQ_ID_COMPOSITE_RE = re.compile(
    r"\b(?P<sys>[A-Z]{2,12})[-_ ](?P<prefix>REQ|SR|SWR|SYS|SS|SRS|FR|PR)"
    r"[-_ ](?P<num>\d{1,6}(?:\.\d{1,6})*)\b",
    re.IGNORECASE,
)

MEASURE_RE = re.compile(
    r"(\bwithin\s+\d+(\.\d+)?\s*(ms|s|sec|seconds|minutes|hours)\b|<=|>=|<|>|±|"
    r"\baccuracy\b|\blatency\b|\bthroughput\b|\brange\b|\bmtbf\b|\bfps\b|\bhz\b|"
    r"\bknots\b|\bm/s\b|\btops\b|\bgb\b|\bms\b|\bdb\b|\bkm\b|\bmeters?\b|\b°c\b)",
    re.IGNORECASE,
)
NUMBER_RE = re.compile(r"\b\d+(\.\d+)?\b", re.IGNORECASE)
HEDGE_RE = re.compile(r"\b(as appropriate|as needed|if possible|where feasible|etc\.)\b", re.IGNORECASE)
DEFINITION_RE = re.compile(r"\b(denotes|means|is defined as|refers to)\b", re.IGNORECASE)
META_DOC_RE = re.compile(r"\b(this document|this section|this specification|this sow|this statement)\b", re.IGNORECASE)

VERIFY_RE = re.compile(
    r"\b(verify|verification|validated|validation|test|analysis|inspection|demonstration)\b",
    re.IGNORECASE,
)
INTERFACE_RE = re.compile(
    r"\b(interface|interoperate|compatible with|integrate with|connect to)\b",
    re.IGNORECASE,
)

SUBJECT_SYSTEM_RE = re.compile(
    r"\b(the system|the software|the device|the platform|the application|the subsystem)\b",
    re.IGNORECASE,
)
SUBJECT_CONTRACT_RE = re.compile(
    r"\b(the contractor|the vendor|the offeror|the supplier|the provider)\b",
    re.IGNORECASE,
)
SUBJECT_COMPONENT_RE = re.compile(
    r"\b("
    r"(navigation|communications|compute|processing|vision|payload|sensor|eo|ir|eo/ir|rf|emcon)\s*"
    r"(subsystem|system|module|pipeline|algorithm|payload)"
    r"|the\s+(gcs|ground\s+control\s+station|ground\s+station)"
    r"|the\s+(eo|ir)\s+sensor"
    r"|the\s+threat\s+detection\s+algorithm"
    r"|the\s+vision\s+processing\s+pipeline"
    r"|the\s+onboard\s+edge\s+ai\s+accelerator"
    r")\b",
    re.IGNORECASE,
)

CONSTRAINT_WORD_RE = re.compile(
    r"\b(threshold|objective|parameter|constraint|limit|rate|frequency|latency|accuracy)\b",
    re.IGNORECASE,
)
STRUCTURED_LINE_RE = re.compile(r"^([A-Z][A-Za-z0-9/\- ]{2,}?)\s*(\([^)]+\))?\s*[:\-–—]\s+.+\S$")
TABLE_FIGURE_TITLE_RE = re.compile(r"^\s*(table|figure)\s+\d+([.-]\d+)*\s*[:\-–—]", re.IGNORECASE)
MODE_SECTION_RE = re.compile(r"\b(emcon|mode|modes|operating mode)\b", re.IGNORECASE)
MODE_WORD_RE = re.compile(
    r"\b(enabled|disabled|active|inactive|autonomous|streaming|telemetry|video|radio|rf|emission|link)\b",
    re.IGNORECASE,
)

PROCESS_WORDS = re.compile(
    r"\b(program\s+manager|contracting\s+officer|cor\b|personnel|staff\b|attend\b|"
    r"submit\s+(a\s+)?(report|plan|schedule)|coordinate\s+with|notify\s+the|document\s+the|"
    r"provide\s+a\s+(report|plan|schedule|briefing)|cdrl|data\s+item\s+description|did\b)\b",
    re.IGNORECASE,
)
NOISE_RE = re.compile(
    r"^(note[:\s]|this\s|it\s+must\s+be\s+noted|as\s+noted|see\s+|refer\s+to|"
    r"figure\s+|table\s+|section\s+|appendix\s+)",
    re.IGNORECASE,
)

# Definitional boilerplate patterns.
META_PATTERNS = [
    re.compile(r"\bshall\b.{0,40}\bdenotes?\b", re.IGNORECASE),
    re.compile(r"\bshall\b.{0,30}\b(indicates?|means?|refers?\s+to)\b", re.IGNORECASE),
    re.compile(
        r"\bshall\s+satisfy\s+all\s+requirements?\s+(identified|listed|contained|specified|described|in\s+this)",
        re.IGNORECASE,
    ),
    re.compile(r"\ball\s+requirements?\s+(in|of|within)\s+this\b", re.IGNORECASE),
    re.compile(r"\brequirements?\s+shall\s+be\s+(verified|validated|tested)\s+by\b", re.IGNORECASE),
    re.compile(
        r"^(this\s+(section|document|table|figure|paragraph)\s+(defines?|describes?|lists?|specifies?|contains?|provides?))",
        re.IGNORECASE,
    ),
]


def normalize_ws(text: str) -> str:
    text = text.replace("\u00a0", " ")
    return re.sub(r"\s+", " ", text).strip()


def clean_sentence(text: str) -> str:
    text = normalize_ws(text)
    if not text:
        return ""
    text = re.sub(
        r"^(requirement|req|context|note|rationale|description|title|id|ref)\s*[:\-]\s*",
        "",
        text,
        flags=re.IGNORECASE,
    )
    # Only strip leading IDs with requirement-like prefixes.
    text = re.sub(
        r"^(REQ|SR|SWR|SYS|SS|SRS|FR|PR|APSDS)[-_ ]\d+(\.\d+)?[\s:\-]+",
        "",
        text,
        flags=re.IGNORECASE,
    )
    text = re.sub(r"\s*context\s*:\s*.+$", "", text, flags=re.IGNORECASE)
    return text.strip()


def extract_req_id(text: str, cfg: ExtractConfig) -> Optional[str]:
    text = normalize_ws(text)
    m2 = REQ_ID_COMPOSITE_RE.search(text)
    if m2:
        sys_name = m2.group("sys").upper()
        prefix = m2.group("prefix").upper()
        number = m2.group("num")
        return f"{sys_name}-{prefix}-{number}"

    m = REQ_ID_PREFIX_RE.search(text)
    if m:
        prefix = m.group("prefix").upper()
        if prefix not in {p.upper() for p in cfg.id_prefixes}:
            return None
        return f"{prefix}-{m.group('num')}"

    return None


def stable_id(text: str, section_title: str, section_ref: str) -> str:
    base = f"{section_title} | {section_ref} :: {normalize_ws(text)}"
    digest = hashlib.sha1(base.encode("utf-8")).hexdigest()[:12]
    return f"REQ-{digest}"


def detect_modality(text: str, cfg: ExtractConfig) -> Optional[str]:
    for name, rx in MODAL_PATTERNS:
        if name == "will" and not cfg.treat_will_as_normative:
            continue
        if rx.search(text):
            return name
    return None


def split_compound(text: str) -> List[str]:
    modals = list(SHALL_RE.finditer(text))
    if len(modals) < 2:
        return [text]

    between_start = modals[0].end()
    between_end = modals[1].start()
    between = text[between_start:between_end]

    semi = text.find(";", between_start, between_end)
    if semi != -1:
        part1 = text[:semi].strip().rstrip(",; ")
        part2 = text[semi + 1 :].strip()
        if len(part1) > 15 and len(part2) > 15:
            return [part1, part2]

    and_match = re.search(r"\band\b", between, re.IGNORECASE)
    if and_match:
        split_pos = between_start + and_match.start()
        part1 = text[:split_pos].strip().rstrip(", ")
        part2 = text[split_pos + len(and_match.group()) :].strip()
        if len(part1) > 15 and len(part2) > 15 and SHALL_RE.search(part2):
            return [part1, part2]

    return [text]


def is_definitional(text: str) -> bool:
    if not DEFINITION_RE.search(text):
        return False
    return bool(re.search(r"\bshall\b|\bmust\b|\bshould\b|\bwill\b", text, re.IGNORECASE))


def is_meta_statement(text: str) -> bool:
    for pattern in META_PATTERNS:
        if pattern.search(text):
            return True
    return False


def is_meta_document_statement(text: str) -> bool:
    return bool(META_DOC_RE.search(text))


def classify(text: str, context: Dict[str, Any]) -> str:
    section_title = str(context.get("section_title", ""))
    in_mode_region = bool(MODE_SECTION_RE.search(section_title))

    if SUBJECT_CONTRACT_RE.search(text):
        return "contractual"

    if SUBJECT_SYSTEM_RE.search(text) or SUBJECT_COMPONENT_RE.search(text):
        if INTERFACE_RE.search(text):
            return "interface"
        if VERIFY_RE.search(text):
            return "verification"
        if MEASURE_RE.search(text) or CONSTRAINT_WORD_RE.search(text):
            return "constraint"
        return "system"

    if INTERFACE_RE.search(text):
        return "interface"
    if VERIFY_RE.search(text):
        return "verification"
    if in_mode_region and MODE_WORD_RE.search(text):
        return "constraint"
    if MEASURE_RE.search(text) or CONSTRAINT_WORD_RE.search(text):
        return "constraint"

    return "unknown"


def score_modal(text: str, modality: str, context: Dict[str, Any], cfg: ExtractConfig) -> Tuple[float, Dict[str, float]]:
    score = 0.0
    feats: Dict[str, float] = {}

    if modality in ("shall", "shall_not", "must", "must_not", "required"):
        score += 3.0
        feats["strong_modality"] = 1.0
    elif modality in ("should", "may_not"):
        score += 1.5
        feats["medium_modality"] = 1.0
    elif modality == "will":
        score += 0.5
        feats["weak_modality"] = 1.0

    if SUBJECT_SYSTEM_RE.search(text) or SUBJECT_CONTRACT_RE.search(text) or SUBJECT_COMPONENT_RE.search(text):
        score += 1.0
        feats["has_subject_phrase"] = 1.0

    if MEASURE_RE.search(text) or NUMBER_RE.search(text):
        score += 2.0
        feats["measurable"] = 1.0

    if VERIFY_RE.search(text):
        score += 0.5
        feats["verification_cue"] = 1.0

    if HEDGE_RE.search(text):
        score -= 1.5
        feats["hedge_penalty"] = 1.0

    if PROCESS_WORDS.search(text):
        score -= 0.8
        feats["process_language"] = 1.0

    if NOISE_RE.match(text):
        score -= 1.0
        feats["noise_prefix"] = 1.0

    if extract_req_id(text, cfg):
        score += 1.0
        feats["req_id"] = 1.0

    section_title = str(context.get("section_title", ""))
    if section_title and MODE_SECTION_RE.search(section_title):
        score += 0.4
        feats["mode_section_context"] = 1.0

    words = text.split()
    if 6 <= len(words) <= 45:
        score += 0.5
        feats["good_length"] = 1.0
    elif len(words) > 80:
        score -= 0.6
        feats["too_long"] = 1.0
    elif len(words) < 5:
        score -= 0.8
        feats["too_short"] = 1.0

    if SPACY_OK and _NLP is not None and cfg.use_spacy:
        doc = _NLP(text)
        if any(token.dep_ == "nsubj" for token in doc):
            score += 0.4
            feats["has_subject"] = 1.0
        if any(token.tag_ == "MD" for token in doc):
            score += 0.2
            feats["modal_pos_confirmed"] = 1.0

    return score, feats


def score_non_modal(text: str, context: Dict[str, Any]) -> Tuple[float, Dict[str, float]]:
    score = 0.0
    feats: Dict[str, float] = {}

    if STRUCTURED_LINE_RE.match(text):
        score += 2.0
        feats["structured_line"] = 1.0

    if MEASURE_RE.search(text) or NUMBER_RE.search(text):
        score += 2.5
        feats["measurable"] = 1.0

    if CONSTRAINT_WORD_RE.search(text):
        score += 1.0
        feats["constraint_word"] = 1.0

    section_title = str(context.get("section_title", ""))
    if MODE_SECTION_RE.search(section_title) and MODE_WORD_RE.search(text):
        score += 1.5
        feats["mode_section_boost"] = 1.0

    if HEDGE_RE.search(text):
        score -= 1.0
        feats["hedge_penalty"] = 1.0

    return score, feats


def score_to_confidence(score: float, threshold: float) -> float:
    # Baseline confidence at threshold, then smooth increase above threshold.
    margin = score - threshold
    conf = 0.20 + 0.10 * max(0.0, margin)
    return max(0.0, min(1.0, conf))


def confidence_label(score: float, threshold: float) -> str:
    if score >= threshold + 2.0:
        return "high"
    if score >= threshold:
        return "medium"
    return "low"


def normalize_ui_score(score: float, threshold: float) -> float:
    # Map around threshold into [0, 1] while preserving rank.
    normalized = (score - (threshold - 2.0)) / 5.0
    return max(0.0, min(1.0, round(normalized, 3)))


def generate_name(text: str, modality: str) -> str:
    """
    Derive a specific, descriptive name from a requirement sentence.

    Strategy:
    1. Strip leading condition clause ("When X, " / "If X, ").
    2. Identify the subject — keep domain nouns, drop generic subjects like
       "the system" / "it".  Strip the modal verb.
    3. Strip common action adverbs ("automatically", "immediately", etc.).
    4. Take verb phrase core + key object.  If a number is present within
       the first 10 tokens, anchor the name around it.
    5. Trim trailing connectors, prepositions, and articles.
    6. Cap at 6 words, Title Case.
    """
    _TRAILING_STOP = re.compile(
        r"^(a|an|the|of|in|on|at|to|for|with|within|from|into|"
        r"by|between|and|or|that|which|its|their|this|these|those|"
        r"upon|when|if|as|during|under|over|above|below|per|"
        r"automatically|immediately|properly|correctly|successfully)$",
        re.IGNORECASE,
    )
    _LEADING_ART = re.compile(r"^(a|an|the)\s+", re.IGNORECASE)
    _ADVERB_FILLER = re.compile(
        r"\b(automatically|immediately|properly|correctly|successfully|"
        r"securely|efficiently|seamlessly|dynamically|continuously)\b\s*",
        re.IGNORECASE,
    )
    NUMBER_RE = re.compile(
        r"\b\d+(?:[.,]\d+)?\s*(?:ms|s|sec(?:ond)?s?|min(?:ute)?s?|hours?|"
        r"fps|hz|mbps|gbps|kb|mb|gb|%|percent|degrees?|times?|x\b)?",
        re.IGNORECASE,
    )

    clean = text.strip().rstrip(".")

    # 1. Strip leading condition clause: "When/If/Upon ..., "
    cond = re.match(
        r"^(?:when|if|upon|after|once)\b[^,]{3,100},\s*", clean, re.IGNORECASE
    )
    if cond:
        clean = clean[cond.end():]

    # 2. Extract subject + strip modal.
    #    Generic subjects ("the system", "it", "the device", etc.) are dropped.
    #    Domain subjects ("the insulin pump", "the EHR system", etc.) are kept,
    #    abbreviated to avoid repeating parenthetical expansions.
    GENERIC_SUBJ = re.compile(
        r"^(?:the\s+(?:system|device|software|application|platform|tool|"
        r"module|component|interface|server|client|database|product|solution)|it)\s+"
        r"(?:shall|must|should|will|required\s+to)\s+",
        re.IGNORECASE,
    )
    DOMAIN_SUBJ = re.compile(
        r"^((?:the\s+)?[\w][\w\s\-]{1,35}?)\s+"
        r"(?:shall|must|should|will|required\s+to)\s+",
        re.IGNORECASE,
    )

    subject = ""
    if GENERIC_SUBJ.match(clean):
        clean = clean[GENERIC_SUBJ.match(clean).end():]
    else:
        dm = DOMAIN_SUBJ.match(clean)
        if dm:
            raw_subj = dm.group(1).strip()
            # Drop parenthetical expansions like "(EHR)"
            raw_subj = re.sub(r"\s*\(.*?\)", "", raw_subj)
            # Remove leading article
            raw_subj = _LEADING_ART.sub("", raw_subj).strip()
            # Keep max 3 words from subject
            subj_words = raw_subj.split()[:3]
            subject = " ".join(subj_words)
            clean = clean[dm.end():]
        else:
            # No modal found — strip just the modal if present mid-sentence
            mm = re.search(r"\b(?:shall|must|should|will)\b\s*", clean, re.IGNORECASE)
            if mm:
                clean = clean[mm.end():]

    # 3. Strip filler adverbs from action phrase
    clean = _ADVERB_FILLER.sub("", clean).strip()

    # 4. Build candidate = subject + action phrase, tokenise
    candidate = (subject + " " + clean).strip() if subject else clean
    tokens = [w.strip(".,;:()[]\"'\u2019\u2018\u201c\u201d") for w in candidate.split()]
    tokens = [t for t in tokens if t]

    # 5. Find first number within 10 tokens
    num_idx = None
    for idx, tok in enumerate(tokens[:10]):
        if NUMBER_RE.search(tok):
            num_idx = idx
            break

    _COORD = re.compile(r"^(and|or|but|nor)$", re.IGNORECASE)

    if num_idx is not None and num_idx <= 8:
        # Keep up to 4 content words before number + number + optional unit token
        prefix_tokens = []
        for t in tokens[:num_idx]:
            if _COORD.match(t):
                break  # stop at coordinator before the number
            if not _TRAILING_STOP.match(t):
                prefix_tokens.append(t)
        prefix_tokens = prefix_tokens[:4]
        num_token = tokens[num_idx]
        # Check if next token is a unit word
        unit_token = []
        if num_idx + 1 < len(tokens):
            next_tok = tokens[num_idx + 1]
            if re.match(r"^(?:ms|sec(?:ond)?s?|min(?:ute)?s?|hours?|fps|hz|"
                        r"mbps|gbps|kb|mb|gb|%|percent|degrees?|times?)$",
                        next_tok, re.IGNORECASE):
                unit_token = [next_tok]
        name_tokens = prefix_tokens + [num_token] + unit_token
    else:
        # No number — collect up to 6 content words, stop at coordinator
        name_tokens = []
        for t in tokens:
            if _COORD.match(t):
                break
            if not _TRAILING_STOP.match(t):
                name_tokens.append(t)
            if len(name_tokens) >= 6:
                break

    # 6. Trim trailing stop-words / connectors
    while name_tokens and _TRAILING_STOP.match(name_tokens[-1]):
        name_tokens.pop()

    while name_tokens and name_tokens[-1].lower() in ("and", "or", "with", "to", "the", "a", "an"):
        name_tokens.pop()

    if not name_tokens:
        name_tokens = text.split()[:5]

    # 7. Strip leading article from first token
    first = _LEADING_ART.sub("", name_tokens[0]).strip()
    if first:
        name_tokens[0] = first

    return " ".join(name_tokens).title()


def normalize_input_blocks(payload: Dict[str, Any]) -> List[Dict[str, Any]]:
    raw_blocks = payload.get("blocks", [])
    if isinstance(raw_blocks, list) and raw_blocks:
        normalized: List[Dict[str, Any]] = []
        for item in raw_blocks:
            if isinstance(item, str):
                text = item.strip()
                if text:
                    normalized.append(
                        {
                            "text": text,
                            "section_title": "",
                            "section_ref": "",
                            "section_type": "paragraph",
                            "line_index": -1,
                        }
                    )
                continue

            if not isinstance(item, dict):
                continue

            text = str(item.get("text", "")).strip()
            if not text:
                continue

            normalized.append(
                {
                    "text": text,
                    "section_title": str(item.get("section_title", item.get("sectionTitle", ""))),
                    "section_ref": str(item.get("section_ref", item.get("sectionRef", ""))),
                    "section_type": str(item.get("section_type", item.get("sectionType", "paragraph"))).lower(),
                    "line_index": item.get("line_index", item.get("lineIndex", -1)),
                }
            )
        return normalized

    raw_sentences = payload.get("sentences", [])
    normalized = []
    if isinstance(raw_sentences, list):
        for sentence in raw_sentences:
            text = str(sentence).strip()
            if not text:
                continue
            normalized.append(
                {
                    "text": text,
                    "section_title": "",
                    "section_ref": "",
                    "section_type": "paragraph",
                    "line_index": -1,
                }
            )
    return normalized


def deduplicate(results: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    def tokens(text: str) -> set[str]:
        return set(re.findall(r"\b\w+\b", text.lower()))

    accepted: List[Dict[str, Any]] = []
    for result in results:
        current = tokens(result["sentence"])
        duplicate = False
        for prev in accepted:
            prev_tokens = tokens(prev["sentence"])
            if not current or not prev_tokens:
                continue
            overlap = len(current & prev_tokens) / len(current | prev_tokens)
            if overlap >= 0.75:
                duplicate = True
                break

        result["duplicate"] = duplicate
        if not duplicate:
            accepted.append(result)

    return results


def process(payload: Dict[str, Any]) -> Dict[str, Any]:
    doc_type = str(payload.get("doc_type", "")).strip()
    entries = normalize_input_blocks(payload)
    results: List[Dict[str, Any]] = []

    for entry in entries:
        sentence = clean_sentence(str(entry.get("text", "")))
        if not sentence:
            continue

        parts = split_compound(sentence) if CFG.enable_atomic_split else [sentence]
        for part in parts:
            part = normalize_ws(part)
            if not part:
                continue

            section_title = str(entry.get("section_title", ""))
            section_ref = str(entry.get("section_ref", ""))
            section_type = str(entry.get("section_type", "paragraph"))

            modality = detect_modality(part, CFG)
            req_id = extract_req_id(part, CFG) or stable_id(part, section_title, section_ref)

            if modality:
                if is_definitional(part):
                    continue
                if is_meta_statement(part):
                    continue
                if is_meta_document_statement(part) and not (
                    SUBJECT_SYSTEM_RE.search(part)
                    or SUBJECT_CONTRACT_RE.search(part)
                    or SUBJECT_COMPONENT_RE.search(part)
                ):
                    continue

                raw_score, feats = score_modal(part, modality, entry, CFG)
                if raw_score < CFG.min_accept_score:
                    continue

                confidence_score = score_to_confidence(raw_score, CFG.min_accept_score)
                confidence = confidence_label(raw_score, CFG.min_accept_score)
                classification = classify(part, entry)
                flags = list(feats.keys())
                if modality == "will":
                    flags.append("will_obligation")

                results.append(
                    {
                        "sentence": part,
                        "name": generate_name(part, modality),
                        "score": normalize_ui_score(raw_score, CFG.min_accept_score),
                        "confidence": confidence,
                        "confidence_score": round(confidence_score, 3),
                        "flags": sorted(set(flags)),
                        "classification": classification,
                        "req_id": req_id,
                        "section_title": section_title,
                        "section_ref": section_ref,
                        "section_type": section_type,
                        "duplicate": False,
                    }
                )
                continue

            if not CFG.enable_non_modal_constraints:
                continue
            if TABLE_FIGURE_TITLE_RE.match(part):
                continue

            in_mode_region = bool(MODE_SECTION_RE.search(section_title))
            candidate = (
                STRUCTURED_LINE_RE.match(part)
                or MEASURE_RE.search(part)
                or CONSTRAINT_WORD_RE.search(part)
                or (in_mode_region and MODE_WORD_RE.search(part))
            )
            if not candidate:
                continue

            raw_score, feats = score_non_modal(part, entry)
            if raw_score < CFG.min_non_modal_score:
                continue

            if in_mode_region:
                classification = "constraint"
            elif MEASURE_RE.search(part) or CONSTRAINT_WORD_RE.search(part):
                classification = "constraint"
            else:
                classification = classify(part, entry)

            confidence_score = score_to_confidence(raw_score, CFG.min_non_modal_score)
            confidence = confidence_label(raw_score, CFG.min_non_modal_score)
            flags = list(feats.keys()) + ["implicit_constraint"]

            results.append(
                {
                    "sentence": part,
                    "name": generate_name(part, "implicit"),
                    "score": normalize_ui_score(raw_score, CFG.min_non_modal_score),
                    "confidence": confidence,
                    "confidence_score": round(confidence_score, 3),
                    "flags": sorted(set(flags)),
                    "classification": classification,
                    "req_id": req_id,
                    "section_title": section_title,
                    "section_ref": section_ref,
                    "section_type": section_type,
                    "duplicate": False,
                }
            )

    results.sort(key=lambda r: r["score"], reverse=True)
    results = deduplicate(results)
    return {"results": results, "spacy_available": SPACY_OK, "doc_type": doc_type}


def main() -> None:
    logging.info("req_parser ready, waiting for input on stdin")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
            output = process(payload)
            print(json.dumps(output), flush=True)
        except json.JSONDecodeError as exc:
            print(json.dumps({"error": f"JSON parse error: {exc}"}), flush=True)
        except Exception as exc:
            logging.exception("Unhandled error")
            print(json.dumps({"error": str(exc)}), flush=True)


if __name__ == "__main__":
    main()
