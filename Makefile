SUBDIRS := $(wildcard ./grammars/*/.)
TOPTARGETS := all clean

any: help

help: ## Print this help message
	@grep -E '^[a-zA-Z._-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

$(TOPTARGETS): $(SUBDIRS)
$(SUBDIRS):
	$(MAKE) -C $@ $(MAKECMDGOALS)

prepare:
	find . -iname 'libtree-sitter-*.so' -type f -exec mv {} ./output/ \;

.PHONY: any help all $(TOPTARGETS) $(SUBDIRS)
