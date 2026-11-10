## Autonomous GitHub Issue Resolution Protocol
1. **Discover & Inspect:** Check open issues using `gh issue list --repo srlegrand/Aztek`. Read full descriptions and comments with `gh issue view <issue_id> --comments`.
2. **Context & Investigation:** Review relevant files in the workspace, run existing tests or build commands, and verify current state before touching code.
3. **Implementation:** Apply clean, minimal changes strictly addressing the issue.
4. **Validation:** Run project verification/tests to confirm no regressions.
5. **Git Push:** Stage files, commit with a descriptive message referencing the issue (e.g. `fix: resolve issue #X - <summary>`), and push directly to `origin main` (or the active default branch):
   `git add -A && git commit -m "fix(#X): <description>" && git push origin HEAD`
6. **Close Issue:** Mark the issue resolved via `gh issue close <issue_id> --comment "Implemented and pushed in commit $(git rev-parse --short HEAD)"`.
