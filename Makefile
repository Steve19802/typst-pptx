SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

OXML := tools/validate/oxml
PPTX ?=
PHASE ?= phase0
TARGET ?= x86_64-unknown-linux-musl
# Same versions as the release workflow, read from it.
RELEASE_YML := .github/workflows/release.yml
ZIG_VERSION := $(shell sed -n 's/^  ZIG_VERSION: //p' $(RELEASE_YML))
CARGO_ZIGBUILD_VERSION := $(shell sed -n 's/^  CARGO_ZIGBUILD_VERSION: //p' $(RELEASE_YML))
# Tools for zigbuild live in the project, not the container (it's rebuilt).
RELEASE_TOOLS := tools/release
# Docker network shared with the agent container (see README).
NETWORK ?= typst-pptx

.PHONY: help devcontainer doctor setup build test fmt fmt-check clippy validate-oxml validate milestone milestone-report check zigbuild clean

help: ## List targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-16s %s\n", $$1, $$2}'

devcontainer: ## On the host: (re)create the devcontainer on NETWORK (default typst-pptx)
	docker network inspect $(NETWORK) >/dev/null 2>&1 || docker network create $(NETWORK)
	TYPST_PPTX_DEV_NETWORK=$(NETWORK) devcontainer up --workspace-folder . --remove-existing-container

doctor: ## Print the version of every tool in the dev environment
	@cargo --version
	@rustc --version
	@cargo nextest --version | sed -n 1p
	@cargo insta --version
	@typst --version
	@soffice --version
	@echo "dotnet $$(dotnet --version)"
	@uv --version
	@python3 --version
	@xmllint --version 2>&1 | sed -n 1p

setup: ## Install validator dependencies (Python venv, NuGet packages)
	uv sync --project tools/validate
	dotnet restore $(OXML)

build: ## Build all crates
	cargo build --workspace --all-targets

test: ## Run unit, snapshot and doc tests
	cargo nextest run --workspace --no-tests=pass
	cargo test --workspace --doc

fmt: ## Format the code
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

validate-oxml: ## Open XML SDK check: make validate-oxml PPTX="out/a.pptx out/b.pptx"
	dotnet run --project $(OXML) -c Release -- $(PPTX)

validate: ## Validators + LibreOffice visual diff over the fixture decks (out/validate)
	cargo build --release
	tools/validate/run.sh

milestone: validate ## Zip for the Windows check: make milestone PHASE=phase0
	tools/milestone.sh $(PHASE)

milestone-report: ## Compare PowerPoint's PDFs with Typst's: make milestone-report DIR=out/milestone/...
	tools/milestone_report.sh $(DIR)

check: fmt-check clippy test ## Format, lint and tests

zigbuild: ## Release build as in CI: make zigbuild TARGET=armv7-unknown-linux-musleabi
	test -x $(RELEASE_TOOLS)/venv/bin/python3 || uv venv -q $(RELEASE_TOOLS)/venv
	uv pip install -q --python $(RELEASE_TOOLS)/venv "ziglang==$(ZIG_VERSION)"
	test -x $(RELEASE_TOOLS)/bin/cargo-zigbuild || cargo binstall -y --quiet --root $(RELEASE_TOOLS) "cargo-zigbuild@$(CARGO_ZIGBUILD_VERSION)"
	rustup target add $(TARGET)
	PATH="$$PWD/$(RELEASE_TOOLS)/venv/bin:$$PWD/$(RELEASE_TOOLS)/bin:$$PATH" cargo zigbuild --release --locked -p typst-pptx --target $(TARGET)
	@echo "target/$(TARGET)/release/typst-pptx"

clean: ## Remove build output (keeps out/milestone, which holds Windows results)
	cargo clean
	rm -rf out/validate out/calibration out/compare* out/scratch
