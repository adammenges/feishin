## General guidelines

- Never use the em dash "—". Use plain dash "-" instead.
- Stage and commit only on an explicit commit request. Run the relevant native checks from `docs/native/README.md` before committing; do not stage outside a commit request.

## Agent skills

### Issue tracker

Issues live as markdown under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.

### Architecture

Native crate boundaries and the embedded browser remote: read `docs/agents/architecture.md` when crossing processes or builds.

### API

Server library data layer (core API clients, domain models and normalization): read `docs/agents/api.md` when adding or changing fetches, mutations, or backend mapping. Consult upstream API contracts only as a last resort when in-repo methods and models are insufficient.

### Frontend

Native GPUI views, state, themes, translations and controls: read `docs/agents/frontend.md` when writing or changing frontend UI, styles, or client state.

### Logging

App logging (Rust `log` macros and redacted diagnostics): read `docs/agents/logging.md` when adding or changing diagnosable code paths - failures, fallbacks, engine/connection lifecycle.

### Commits

Conventional Commits and product scopes: read `docs/agents/commits.md` when writing a commit message.

## Working mode

Ponytail (lazy senior / YAGNI ladder) on coding tasks. See the `ponytail` skill.
