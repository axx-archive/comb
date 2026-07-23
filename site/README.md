# Comb launch site

Public launch site for Comb, the independent open-source organizational-memory layer for Buzz built by That’s Cool.

## Run locally

Requires Node.js 22.13 or newer.

```bash
npm ci
npm run dev
```

The preview opens at `http://localhost:3000`.

## Validate

```bash
npm test
npm run lint
npx tsc --noEmit
```

`npm test` creates the production vinext build and exercises the rendered HTML, truth-status manifest, metadata, accessibility contracts, independent-project disclaimer, and required brand assets.

## Truth and branding contracts

- `app/project-status.ts` is the source of truth for what works in Comb, what was tested against Buzz, and what is merely proposed upstream.
- `public/thatscool-maker.png` is the canonical That’s Cool maker sticker from the lab repository.
- `public/og.png` is the 1731 × 909 social card.
- Compatibility proof claims must match `../tests/e2e/buzz-main-proof.json`.
- The site must not imply affiliation with or endorsement by Block.

## Hosting

The site is built with vinext and deployed through OpenAI Sites. `.openai/hosting.json` holds only the opaque project identifier and optional resource bindings; runtime secrets do not belong in the repository.
