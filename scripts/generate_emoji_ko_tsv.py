#!/usr/bin/env python3
"""Generate emoji_ko.tsv from Unicode CLDR Korean annotations (ko.xml).

Parses the CLDR annotations file and creates a TSV with:
  emoji\tkorean_name\tkorean_keywords

Only includes emoji that exist in our base emoji.tsv data.
"""

import urllib.request
import xml.etree.ElementTree as ET
import os

CLDR_KO_URL = "https://raw.githubusercontent.com/unicode-org/cldr/main/common/annotations/ko.xml"
CLDR_KO_DERIVED_URL = "https://raw.githubusercontent.com/unicode-org/cldr/main/common/annotationsDerived/ko.xml"

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(SCRIPT_DIR, "..", "crates", "kmd-core", "data")
BASE_TSV = os.path.join(DATA_DIR, "emoji.tsv")
OUTPUT_TSV = os.path.join(DATA_DIR, "emoji_ko.tsv")


def load_base_emoji():
    """Load the set of emoji from our base emoji.tsv."""
    emoji_set = set()
    with open(BASE_TSV, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            emoji = line.split("\t")[0].strip()
            if emoji:
                emoji_set.add(emoji)
    return emoji_set


def parse_cldr_annotations(url):
    """Parse a CLDR annotations XML file and return dict: emoji -> (tts_name, keywords_list)."""
    print(f"  Fetching {url} ...")
    response = urllib.request.urlopen(url)
    content = response.read()
    
    root = ET.fromstring(content)
    
    annotations = {}  # emoji -> {"tts": str, "keywords": [str]}
    
    for ann in root.iter("annotation"):
        cp = ann.get("cp", "")
        if not cp:
            continue
        
        text = (ann.text or "").strip()
        if not text:
            continue
        
        is_tts = ann.get("type") == "tts"
        
        if cp not in annotations:
            annotations[cp] = {"tts": "", "keywords": []}
        
        if is_tts:
            annotations[cp]["tts"] = text
        else:
            # Keywords are separated by " | "
            kws = [kw.strip() for kw in text.split("|") if kw.strip()]
            annotations[cp]["keywords"] = kws
    
    return annotations


def main():
    print("Loading base emoji set...")
    base_emoji = load_base_emoji()
    print(f"  Found {len(base_emoji)} base emoji")
    
    print("Parsing CLDR annotations...")
    # Main annotations (keywords)
    ann_main = parse_cldr_annotations(CLDR_KO_URL)
    # Derived annotations (tts names for sequences)
    ann_derived = parse_cldr_annotations(CLDR_KO_DERIVED_URL)
    
    # Merge: derived overrides/supplements main
    merged = {}
    for cp in set(list(ann_main.keys()) + list(ann_derived.keys())):
        main_data = ann_main.get(cp, {"tts": "", "keywords": []})
        derived_data = ann_derived.get(cp, {"tts": "", "keywords": []})
        
        tts = derived_data["tts"] or main_data["tts"]
        keywords = main_data["keywords"] or derived_data["keywords"]
        
        merged[cp] = {"tts": tts, "keywords": keywords}
    
    print(f"  Total CLDR entries: {len(merged)}")
    
    # Match with base emoji
    matched = 0
    entries = []
    
    for emoji in sorted(base_emoji):
        if emoji in merged:
            data = merged[emoji]
            tts = data["tts"]
            # Remove the tts name from keywords list to avoid duplication
            kws = [kw for kw in data["keywords"] if kw != tts]
            kw_str = "|".join(kws) if kws else ""
            entries.append(f"{emoji}\t{tts}\t{kw_str}")
            matched += 1
    
    print(f"  Matched {matched}/{len(base_emoji)} emoji with Korean data")
    
    # Write output
    os.makedirs(os.path.dirname(OUTPUT_TSV), exist_ok=True)
    with open(OUTPUT_TSV, "w", encoding="utf-8") as f:
        for entry in entries:
            f.write(entry + "\n")
    
    print(f"  Written to {OUTPUT_TSV}")
    
    # Show a few samples
    print("\nSamples:")
    for entry in entries[:10]:
        parts = entry.split("\t")
        print(f"  {parts[0]} → {parts[1]} [{parts[2][:50]}...]")


if __name__ == "__main__":
    main()
