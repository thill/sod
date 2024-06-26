.PHONY: build
release: # Release version [v: version]
	./scripts/release.sh $(v)
