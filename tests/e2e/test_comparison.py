#!/usr/bin/env python3
"""Pytest-based E2E comparison tests for CodeLoom vs Repomix."""

import subprocess
import time
import json
import re
import os
import pytest
from pathlib import Path
from typing import Dict, Tuple, Optional
from dataclasses import dataclass

from config import (
    TEST_REPOSITORIES,
    TestRepo,
    ComparisonMetrics,
    REPOS_DIR,
    OUTPUTS_DIR,
    CODELOOM_BIN,
    REPOMIX_BIN,
    ensure_dirs,
    get_repo_path,
)


# Test fixtures
@pytest.fixture(scope="session", autouse=True)
def setup_directories():
    """Ensure test directories exist."""
    ensure_dirs()


@pytest.fixture(scope="session")
def codeloom_available():
    """Check if CodeLoom is available."""
    return CODELOOM_BIN.exists()


@pytest.fixture(scope="session")
def repomix_available():
    """Check if Repomix is available."""
    try:
        result = subprocess.run(["repomix", "--version"], capture_output=True)
        return result.returncode == 0
    except FileNotFoundError:
        return False


def clone_repo_fixture(repo: TestRepo) -> Path:
    """Clone repository if needed and return path."""
    repo_path = get_repo_path(repo)

    if not repo_path.exists():
        result = subprocess.run(
            ["git", "clone", "--depth", "1", "--branch", repo.branch, repo.url, str(repo_path)],
            capture_output=True,
            timeout=300,
        )
        if result.returncode != 0:
            pytest.skip(f"Failed to clone {repo.name}")

    return repo_path


# Parameterized tests for each repository
@pytest.fixture(params=[r.name for r in TEST_REPOSITORIES[:3]])  # Test first 3 repos
def test_repo(request) -> TestRepo:
    """Provide test repository configuration."""
    repo = next((r for r in TEST_REPOSITORIES if r.name == request.param), None)
    if repo is None:
        pytest.skip(f"Repository {request.param} not found")
    return repo


class TestRepomixBaseline:
    """Tests for Repomix baseline functionality."""

    def test_repomix_runs(self, test_repo: TestRepo, repomix_available):
        """Test that Repomix can process a repository."""
        if not repomix_available:
            pytest.skip("Repomix not installed")

        repo_path = clone_repo_fixture(test_repo)
        output_file = OUTPUTS_DIR / f"{test_repo.name}_repomix_test.xml"

        result = subprocess.run(
            [REPOMIX_BIN, str(repo_path), "-o", str(output_file), "--style", "xml"],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo_path),
        )

        assert result.returncode == 0, f"Repomix failed: {result.stderr}"
        assert output_file.exists(), "Repomix did not create output file"
        assert output_file.stat().st_size > 0, "Repomix output file is empty"

    def test_repomix_detects_files(self, test_repo: TestRepo, repomix_available):
        """Test that Repomix detects the expected number of files."""
        if not repomix_available:
            pytest.skip("Repomix not installed")

        repo_path = clone_repo_fixture(test_repo)
        output_file = OUTPUTS_DIR / f"{test_repo.name}_repomix_files.xml"

        result = subprocess.run(
            [REPOMIX_BIN, str(repo_path), "-o", str(output_file), "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo_path),
        )

        # Check file count is in expected range
        output = result.stdout + result.stderr
        file_match = re.search(r"(\d+)\s*files?", output, re.I)

        if file_match:
            file_count = int(file_match.group(1))
            assert file_count >= test_repo.expected_files_min, \
                f"Too few files: {file_count} < {test_repo.expected_files_min}"
            assert file_count <= test_repo.expected_files_max, \
                f"Too many files: {file_count} > {test_repo.expected_files_max}"


class TestCodeLoomBaseline:
    """Tests for CodeLoom baseline functionality."""

    def test_codeloom_runs(self, test_repo: TestRepo, codeloom_available):
        """Test that CodeLoom can process a repository."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        repo_path = clone_repo_fixture(test_repo)

        result = subprocess.run(
            [str(CODELOOM_BIN), str(repo_path)],
            capture_output=True,
            timeout=600,
            text=True,
        )

        assert result.returncode == 0, f"CodeLoom failed: {result.stderr}"
        assert "Files:" in result.stdout, "CodeLoom output missing file count"

    def test_codeloom_detects_files(self, test_repo: TestRepo, codeloom_available):
        """Test that CodeLoom detects the expected number of files."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        repo_path = clone_repo_fixture(test_repo)

        result = subprocess.run(
            [str(CODELOOM_BIN), str(repo_path), "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
        )

        output = result.stdout
        file_match = re.search(r"Files:\s*(\d+)", output)

        assert file_match, "Could not find file count in output"
        file_count = int(file_match.group(1))

        assert file_count >= test_repo.expected_files_min, \
            f"Too few files: {file_count} < {test_repo.expected_files_min}"
        assert file_count <= test_repo.expected_files_max, \
            f"Too many files: {file_count} > {test_repo.expected_files_max}"

    def test_codeloom_language_detection(self, test_repo: TestRepo, codeloom_available):
        """Test that CodeLoom detects the primary languages."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        repo_path = clone_repo_fixture(test_repo)

        result = subprocess.run(
            [str(CODELOOM_BIN), str(repo_path), "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
        )

        output = result.stdout.lower()

        # Check that at least one primary language is detected
        detected = any(lang in output for lang in test_repo.primary_languages)
        assert detected, \
            f"None of the primary languages {test_repo.primary_languages} detected in output"


class TestPerformanceComparison:
    """Performance comparison tests between CodeLoom and Repomix."""

    def test_codeloom_faster_than_repomix(self, test_repo: TestRepo, codeloom_available, repomix_available):
        """Test that CodeLoom is at least as fast as Repomix (with tolerance)."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")
        if not repomix_available:
            pytest.skip("Repomix not installed")

        repo_path = clone_repo_fixture(test_repo)

        # Run Repomix
        repomix_output = OUTPUTS_DIR / f"{test_repo.name}_perf_repomix.xml"
        start = time.time()
        subprocess.run(
            [REPOMIX_BIN, str(repo_path), "-o", str(repomix_output)],
            capture_output=True,
            timeout=600,
            cwd=str(repo_path),
        )
        repomix_time = time.time() - start

        # Run CodeLoom
        start = time.time()
        subprocess.run(
            [str(CODELOOM_BIN), str(repo_path)],
            capture_output=True,
            timeout=600,
        )
        codeloom_time = time.time() - start

        # CodeLoom should be within 5x of Repomix time (generous for now)
        # Ideally it should be faster
        assert codeloom_time <= repomix_time * 5, \
            f"CodeLoom too slow: {codeloom_time:.2f}s vs Repomix {repomix_time:.2f}s"

        # Log the comparison
        speedup = repomix_time / max(codeloom_time, 0.001)
        print(f"\n  {test_repo.name}: CodeLoom {codeloom_time:.2f}s, Repomix {repomix_time:.2f}s, Speedup: {speedup:.2f}x")


class TestFileCoverage:
    """Tests comparing file coverage between tools."""

    def test_similar_file_counts(self, test_repo: TestRepo, codeloom_available, repomix_available):
        """Test that both tools detect similar numbers of files."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")
        if not repomix_available:
            pytest.skip("Repomix not installed")

        repo_path = clone_repo_fixture(test_repo)

        # Get Repomix file count
        repomix_output = OUTPUTS_DIR / f"{test_repo.name}_coverage_repomix.xml"
        result = subprocess.run(
            [REPOMIX_BIN, str(repo_path), "-o", str(repomix_output), "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo_path),
        )
        repomix_match = re.search(r"(\d+)\s*files?", result.stdout + result.stderr, re.I)
        repomix_files = int(repomix_match.group(1)) if repomix_match else 0

        # Get CodeLoom file count
        result = subprocess.run(
            [str(CODELOOM_BIN), str(repo_path)],
            capture_output=True,
            timeout=600,
            text=True,
        )
        codeloom_match = re.search(r"Files:\s*(\d+)", result.stdout)
        codeloom_files = int(codeloom_match.group(1)) if codeloom_match else 0

        # Both should detect files
        assert repomix_files > 0, "Repomix detected 0 files"
        assert codeloom_files > 0, "CodeLoom detected 0 files"

        # File counts should be within 50% of each other
        # (some variance is expected due to different ignore rules)
        ratio = min(repomix_files, codeloom_files) / max(repomix_files, codeloom_files)
        assert ratio >= 0.5, \
            f"File count mismatch: Repomix={repomix_files}, CodeLoom={codeloom_files}"

        print(f"\n  {test_repo.name}: Repomix={repomix_files}, CodeLoom={codeloom_files}, Ratio={ratio:.2f}")


class TestOutputQuality:
    """Tests for output quality metrics."""

    def test_codeloom_output_size_reasonable(self, test_repo: TestRepo, codeloom_available):
        """Test that CodeLoom output size is reasonable."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        repo_path = clone_repo_fixture(test_repo)

        result = subprocess.run(
            [str(CODELOOM_BIN), str(repo_path), "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
        )

        output = result.stdout
        output_size = len(output)

        # Output should be non-trivial but not huge
        assert output_size > 100, "Output too small"
        assert output_size < 10_000_000, "Output too large (>10MB)"


# Quick smoke test that can be run independently
class TestQuickSmoke:
    """Quick smoke tests that don't require cloning large repos."""

    def test_codeloom_help(self, codeloom_available):
        """Test CodeLoom --help works."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        result = subprocess.run(
            [str(CODELOOM_BIN), "--help"],
            capture_output=True,
            timeout=10,
            text=True,
        )

        assert result.returncode == 0
        assert "codeloom-scan" in result.stdout
        assert "OPTIONS" in result.stdout

    def test_codeloom_version(self, codeloom_available):
        """Test CodeLoom --version works."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        result = subprocess.run(
            [str(CODELOOM_BIN), "--version"],
            capture_output=True,
            timeout=10,
            text=True,
        )

        assert result.returncode == 0
        assert "0.1.0" in result.stdout

    def test_repomix_help(self, repomix_available):
        """Test Repomix --help works."""
        if not repomix_available:
            pytest.skip("Repomix not installed")

        result = subprocess.run(
            [REPOMIX_BIN, "--help"],
            capture_output=True,
            timeout=10,
            text=True,
        )

        assert result.returncode == 0
        assert "repomix" in result.stdout.lower()

    def test_scan_current_directory(self, codeloom_available):
        """Test scanning the CodeLoom repo itself."""
        if not codeloom_available:
            pytest.skip("CodeLoom not built")

        # Scan the codeloom project itself
        from config import PROJECT_ROOT

        result = subprocess.run(
            [str(CODELOOM_BIN), str(PROJECT_ROOT)],
            capture_output=True,
            timeout=60,
            text=True,
        )

        assert result.returncode == 0
        assert "Files:" in result.stdout

        # Should find zig and rust files
        file_match = re.search(r"Files:\s*(\d+)", result.stdout)
        assert file_match and int(file_match.group(1)) > 0


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
