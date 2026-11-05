#!/usr/bin/env bash
# Writes the files of the 11R5/apix-action repository, the copy of the Action
# listed on the GitHub Marketplace (which wants an action with no workflows
# of its own, so it cannot be this repository):
#   scripts/action-repo.sh <out folder>
# It is this repository's action.yml, except that it always downloads a
# released apix (there are no sources there to build): the release named by
# its own tag (@v0.1.1), else the one in apix-version, else the latest.
set -euo pipefail
mkdir -p "$1"
out="$(cd "$1" && pwd)"
cd "$(dirname "$0")/.."

tr -d '\r' < action.yml | awk '
  # The build-from-source steps: everything from the fingerprint comment up
  # to the step that runs the collection.
  /^    # The cache key is a hash of the sources/ { skip = 1 }
  /^    - name: Run the collection/ { skip = 0 }
  skip { next }
  /^#   - uses: 11R5\/Apix@main/ { sub(/11R5\/Apix@main      # better: a tag or commit/, "11R5/apix-action@v1") }
  /^#     - uses: 11R5\/Apix@main/ { sub(/11R5\/Apix@main/, "11R5/apix-action@v1") }
  /^# At a release tag/ {
    print "# It downloads a released apix, in seconds: the release in apix-version,"
    print "# else the one of its own tag (@v0.1.1), else the latest. The sources and"
    print "# issues are at https://github.com/11R5/Apix."
    drop = 1; next
  }
  drop && /^#/ { next }
  { drop = 0 }
  /^      The apix release to download:/ {
    print "      The apix release to download: \"latest\" or a version (\"0.1.0\"). Empty: the"
    print "      action'"'"'s own release when it is used at a release tag, else the latest"
    getline; next
  }
  /^        echo "version=\$version"/ {
    print "        echo \"version=${version:-latest}\" >> \"$GITHUB_OUTPUT\""
    print "        echo \"repo=11R5/Apix\" >> \"$GITHUB_OUTPUT\""
    getline; next
  }
  { print }
' > "$out/action.yml"

cp LICENSE "$out/LICENSE"
cat > "$out/README.md" <<'EOF'
# Apix run

Sends an [Apix](https://github.com/11R5/Apix) collection's requests in GitHub
Actions, runs its tests and writes JUnit, HTML and Markdown reports. The
summary shows on the job page and, if you like, as one comment on the pull
request that is kept up to date.

```yaml
- uses: actions/checkout@v4
- uses: 11R5/apix-action@v1
  with:
    path: api
    environment: staging
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

Only the requests a pull request touches, with the summary as a comment:

```yaml
permissions: { contents: read, pull-requests: write }
steps:
  - uses: actions/checkout@v4
    with: { fetch-depth: 0 }
  - uses: 11R5/apix-action@v1
    with: { path: api, changed: "true", diff: "true", comment: "true" }
```

The action downloads a released `apix` for Linux, macOS or Windows runners:
the version in `apix-version`, else the one matching the tag you use
(`@v0.1.1`), else the latest. Every input is listed in
[action.yml](action.yml) and explained in the
[workflow guide](https://github.com/11R5/Apix/blob/main/Docs/en/workflows.md).

This repository is generated from the Apix repository by
`scripts/action-repo.sh`; send issues and changes there.

Apache-2.0
EOF
find "$out" -type f
