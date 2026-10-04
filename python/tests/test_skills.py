"""Feature 008: the skills library cannot drift from the release (contracts/skills.md, research R8).

- Every example runs against the installed package and asserts its own outcome.
- Every code excerpt in a SKILL.md is copied verbatim from an example.
- Every API name or command a skill mentions is in the public API manifest.
- Each skill names the release it describes and carries the semantic-gap rule.
- `skills/` contains exactly the three consumer skills; the engine skill lives elsewhere.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SKILLS = ROOT / "skills"
ENGINE_SKILL = ROOT / ".claude" / "skills" / "behavior-engine-development" / "SKILL.md"
MANIFEST = json.loads((ROOT / "api" / "public-api.json").read_text())
RELEASE = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
CONSUMER = ["behavior-application", "behavior-authoring", "behavior-verification"]

GAP_RULE = (
    "If the public Behavior API cannot express a requirement, record a semantic gap in "
    "`SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals, "
    "no hand-written IR, no moving the rule into host code, no silent approximation."
)

PUBLIC_NAMES = set(MANIFEST["python"]["names"])
CLI = set(MANIFEST["cli"])


def run_example(path: Path, tmp: Path) -> tuple[bool, str]:
    """Runs one example as a consumer would: from an empty directory, with only the installed
    package importable. Returns (ok, message naming the file)."""
    env = {k: v for k, v in os.environ.items() if k != "PYTHONPATH"}
    r = subprocess.run([sys.executable, str(path)], cwd=tmp, env=env, capture_output=True,
                       text=True, timeout=600)
    tail = (r.stdout + r.stderr).strip().splitlines()[-3:]
    return r.returncode == 0, f"{path.relative_to(path.parents[2])}: {' | '.join(tail)}"


def frontmatter(text: str) -> dict[str, str]:
    if not text.startswith("---\n"):
        return {}
    head = text.split("\n---\n", 1)[0].splitlines()[1:]
    out: dict[str, str] = {}
    for line in head:
        key, _, value = line.partition(":")
        out[key.strip()] = value.strip()
    return out


def python_blocks(text: str) -> list[list[str]]:
    return [block.splitlines() for block in re.findall(r"```python\n(.*?)```", text, re.S)]


def skill_ids() -> list[str]:
    return sorted(p.name for p in SKILLS.glob("behavior-*") if p.is_dir())


# --- layout -------------------------------------------------------------------------------------


def test_skills_contains_exactly_the_consumer_skills() -> None:
    entries = sorted(p.name for p in SKILLS.iterdir() if not p.name.startswith("."))
    assert entries == sorted(["README.md", "evals", *CONSUMER])
    assert not (SKILLS / "behavior-engine-development").exists()


def test_readme_defines_the_gap_log_and_the_rule() -> None:
    text = (SKILLS / "README.md").read_text()
    assert GAP_RULE in " ".join(line.lstrip("> ").strip() for line in text.splitlines())
    for fixed in ["Date", "Behavior release", "Requirement", "Why inexpressible",
                  "What was done instead", "Severity", "Evidence"]:
        assert f"**{fixed}**" in text, fixed


# --- per consumer skill -------------------------------------------------------------------------


@pytest.mark.parametrize("skill", skill_ids())
def test_frontmatter_names_the_skill_and_the_release(skill: str) -> None:
    fm = frontmatter((SKILLS / skill / "SKILL.md").read_text())
    assert fm.get("name") == skill
    assert fm.get("description")
    assert fm.get("release") == RELEASE


@pytest.mark.parametrize("skill", skill_ids())
def test_the_gap_rule_is_present(skill: str) -> None:
    text = (SKILLS / skill / "SKILL.md").read_text()
    flat = " ".join(line.lstrip("> ").strip() for line in text.splitlines())
    assert GAP_RULE in flat


@pytest.mark.parametrize("skill", skill_ids())
def test_examples_run_against_the_installed_package(skill: str, tmp_path: Path) -> None:
    examples = sorted((SKILLS / skill / "examples").glob("*.py"))
    assert examples, f"{skill} has no examples"
    for example in examples:
        ok, message = run_example(example, tmp_path)
        assert ok, message


@pytest.mark.parametrize("skill", skill_ids())
def test_examples_use_only_the_public_api(skill: str) -> None:
    for example in sorted((SKILLS / skill / "examples").glob("*.py")):
        text = example.read_text()
        assert "behavior._" not in text and "from behavior." not in text, example.name
        assert "examples." not in text and "sys.path" not in text, example.name
        for block in re.findall(r"from behavior import \(?([^)]*?)\)?\n(?!\s)", text, re.S):
            names = {n.strip() for n in block.replace("\n", ",").split(",") if n.strip()}
            assert names <= PUBLIC_NAMES, f"{example.name}: {sorted(names - PUBLIC_NAMES)}"


@pytest.mark.parametrize("skill", skill_ids())
def test_excerpts_are_verbatim_from_examples(skill: str) -> None:
    text = (SKILLS / skill / "SKILL.md").read_text()
    for block in python_blocks(text):
        m = re.match(r"# from (examples/[a-z0-9_]+\.py)$", block[0] if block else "")
        assert m, f"{skill}: excerpt without `# from examples/<file>.py`: {block[:2]}"
        source = (SKILLS / skill / m.group(1)).read_text().splitlines()
        i = 0
        for line in block[1:]:
            while i < len(source) and source[i] != line:
                i += 1
            assert i < len(source), f"{skill}: `{line}` not found (in order) in {m.group(1)}"
            i += 1


def reference_problems(text: str) -> list[str]:
    """API names and commands a skill mentions that are not in the public API manifest."""
    problems = []
    for name in re.findall(r"\bbehavior\.([A-Za-z_][A-Za-z0-9_]*)", text):
        if name not in PUBLIC_NAMES:
            problems.append(f"behavior.{name}")
    code = re.findall(r"`([^`\n]+)`", text) + [
        line for block in re.findall(r"```(?:bash|sh|console)\n(.*?)```", text, re.S)
        for line in block.splitlines()
    ]
    for snippet in code:
        m = re.match(r"\s*(?:\$ )?behavior ([a-z][a-z-]*)", snippet)
        if m and m.group(1) not in CLI:
            problems.append(f"behavior {m.group(1)}")
    for block in python_blocks(text):
        for line in block:
            m = re.match(r"from behavior import ([^(]+)$", line)
            if m:
                problems += sorted({n.strip() for n in m.group(1).split(",")} - PUBLIC_NAMES)
    return problems


@pytest.mark.parametrize("skill", skill_ids())
def test_references_are_in_the_public_api(skill: str) -> None:
    assert reference_problems((SKILLS / skill / "SKILL.md").read_text()) == []


# --- the checks catch drift (SC-002, FR-021) ---------------------------------------------------


def test_a_broken_example_is_reported_by_name(tmp_path: Path) -> None:
    skill = tmp_path / "skills" / "behavior-demo" / "examples"
    skill.mkdir(parents=True)
    broken = skill / "broken_outcome.py"
    broken.write_text("import behavior\nassert behavior.__version__ == 'not-this', 'drifted'\n")
    ok, message = run_example(broken, tmp_path)
    assert not ok
    assert "broken_outcome.py" in message and "drifted" in message


def test_an_unlisted_name_or_a_wrong_release_is_caught() -> None:
    text = (
        "Call `behavior.not_a_real_name()` or run `behavior no-such-command`.\n"
        "```python\n# from examples/x.py\nfrom behavior import select, frobnicate\n```\n"
    )
    assert reference_problems(text) == [
        "behavior.not_a_real_name", "behavior no-such-command", "frobnicate",
    ]
    assert reference_problems("Use `behavior.select` and `behavior verify`.") == []
    assert frontmatter("---\nname: x\nrelease: 0.0.1\n---\n").get("release") != RELEASE


# --- the engine skill (US6) ----------------------------------------------------------------------


def test_the_engine_skill_lives_with_the_core() -> None:
    """Feature 011: guidance for changing the engine moved to behavior-ir-core with the engine;
    this repository keeps only the consumer skills."""
    assert not ENGINE_SKILL.exists()
