+++
id = "arcl-t-01M65H092MYZSYJ3YR5SYQJ2WZ"
phase = "arcl-ph-01M65H08T6XANNJTGAFMMKV0VE"
title = "Release with fabrial's dist flow"
status = "pending"
priority = "normal"
position = 1
blocked-by = ["arcl-t-01M65H093DD9QMJAYD27BA5P28"]
+++

Dalil has not been released and will not go to crates.io. Use fabrial's
release flow instead of the custom `release.yml`.

Acceptance criteria:

- [ ] `dist-workspace.toml` builds Linux x86_64, macOS (both architectures)
      and Windows archives
- [ ] Release PR and Release tag workflows match fabrial's
- [ ] The tag creates the GitHub release and updates the Homebrew tap
- [ ] The README's install section names the release installer and Homebrew
