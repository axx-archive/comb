import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request("http://localhost/", {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}

test("server-renders the complete Comb launch story", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>Comb — DLC for Buzz<\/title>/i);
  assert.match(html, /Your workspace is buzzing\./);
  assert.match(html, /Keep what it learns\./);
  assert.match(html, /Buzz captures the work\./);
  assert.match(html, /Comb keeps the meaning\./);
  assert.match(html, /Ask why\. Get the whole why\./);
  assert.match(html, /Interactive demo \/ illustrative data/i);
  assert.match(html, /Ship the destination\. Upstream the primitives\./);
  assert.match(html, /What if the whole workplace remembered\?/);
  assert.match(html, /Built by/);
  assert.match(html, /thatscool-maker\.png/);
  assert.match(html, /property="og:image" content="http:\/\/localhost\/og\.png"/i);
});

test("renders honest initial delivery status from the manifest", async () => {
  const response = await render();
  const html = await response.text();

  assert.match(html, /Comb core/i);
  assert.match(html, /Working \/ 29 tests/i);
  assert.match(html, /Working in Comb/i);
  assert.match(html, /Compatibility demo/i);
  assert.match(html, /Tested \/ acfbb1b \/ Jul 22, 2026/i);
  assert.match(html, /6\/6 proof passed/i);
  assert.match(html, /Upstream/i);
  assert.match(html, /RFC open \/ #2451/i);
  assert.match(html, /Read Buzz RFC #2451/i);
  assert.match(html, /does not mean these semantics are upstreamed into Buzz/i);
  assert.doesNotMatch(html, /Merged in Buzz|Implementation PR open/i);
});

test("includes the independent-project and accessibility contracts", async () => {
  const response = await render();
  const html = await response.text();

  assert.match(html, /not affiliated with, sponsored by, or endorsed by Block/i);
  assert.match(html, /href="#proof"[^>]*>\s*Skip to the evidence explorer/i);
  assert.match(html, /aria-label="Example questions"/i);
  assert.match(html, /aria-live="polite"/i);
  assert.match(html, /aria-expanded="true"/i);
  assert.match(html, /aria-pressed="true"/i);
  assert.doesNotMatch(html, /codex-preview|react-loading-skeleton|Building your site/i);
});

test("starter preview is removed and maker asset is present", async () => {
  await assert.rejects(access(new URL("../app/_sites-preview/", import.meta.url)));
  await access(new URL("../public/thatscool-maker.png", import.meta.url));
  await access(new URL("../public/og.png", import.meta.url));

  const packageJson = await readFile(new URL("../package.json", import.meta.url), "utf8");
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
});
