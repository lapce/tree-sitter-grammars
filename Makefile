any: help

help: ## Print this help message
	@grep -E '^[a-zA-Z._-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

build:
	python ./make.py

tree-sitter: ## install tree-sitter-cli
	cargo install tree-sitter-cli@^0.22 --locked

.PHONY: any help build
