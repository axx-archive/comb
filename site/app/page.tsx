import Image from "next/image";
import { EvidenceExplorer } from "./evidence-explorer";
import { contributionRows, projectStatus } from "./project-status";

const sourceUrl = "https://github.com/axx-archive/comb";
const buzzUrl = "https://github.com/block/buzz";
const buzzRfcUrl = "https://github.com/block/buzz/issues/2451";
const strideUrl = "https://workinstride.ajh-archive.chatgpt.site/";
const compatibilityProofUrl = `${sourceUrl}/blob/main/tests/e2e/buzz-main-proof.json`;

const capabilityCards = [
  {
    number: "01",
    title: "Receipts, not vibes.",
    body: "Every claim opens to the exact event, transcript span, workflow run, patch, or artifact that supports it.",
  },
  {
    number: "02",
    title: "Truth has a timestamp.",
    body: "Ask what the project believes now—or what the team knew when a decision was made.",
  },
  {
    number: "03",
    title: "Memory respects the room.",
    body: "The compatibility proof keeps evidence channel-local and invalidates dependent memory after source deletion. Broader permission-loss propagation is next.",
  },
  {
    number: "04",
    title: "The company can see itself.",
    body: "The larger vision is to surface living storylines, disagreements, communication gaps, stalled commitments, and the next moves worth making.",
  },
];

const pipeline = ["Talk", "Find the claims", "Check the evidence", "Ratify", "Remember", "Act"];

const buzzPrimitives = [
  "Signed events",
  "Human + agent identities",
  "Channels + huddles",
  "Branches + workflows",
  "Human ratification",
  "Searchable project memory",
];

export default function Home() {
  return (
    <main>
      <a className="skip-link" href="#proof">
        Skip to the evidence explorer
      </a>

      <header className="site-rail" aria-label="Primary navigation">
        <a className="rail-brand" href="#top" aria-label="Comb, back to top">
          <strong>COMB</strong>
          <span>DLC FOR BUZZ*</span>
        </a>
        <nav aria-label="Page links">
          <a href="#proof">Demo</a>
          <a href="#contributions">Contributions</a>
          <a href={sourceUrl} target="_blank" rel="noreferrer">
            Source <span aria-hidden="true">↗</span>
          </a>
        </nav>
        <p>*Independent. Open source. The good kind of unofficial.</p>
      </header>

      <section className="hero" id="top" aria-labelledby="hero-title">
        <div className="hero-copy">
          <p className="eyebrow">Independent open-source DLC for Buzz</p>
          <h1 id="hero-title">
            Your workspace is buzzing. <em>Keep what it learns.</em>
          </h1>
          <p className="hero-intro">
            Comb gives Buzz a permission-aware path from signed channel conversations to ratified
            project memory—with receipts for every claim. Huddles, branches, workflows, and broader
            company intelligence are the roadmap.
          </p>
          <div className="hero-actions">
            <a className="button button-primary" href="#proof">
              Watch it remember
            </a>
            <a className="button button-secondary" href={sourceUrl} target="_blank" rel="noreferrer">
              View the source <span aria-hidden="true">↗</span>
            </a>
          </div>
        </div>

        <div className="hero-system" aria-label="Comb project status">
          <div className="wordmark" aria-hidden="true">
            <span>C</span>
            <span>O</span>
            <span>M</span>
            <span>B</span>
          </div>
          <div className="status-strip">
            {Object.values(projectStatus).map((status) => (
              <div className={`status-cell status-${status.tone}`} key={status.label}>
                <span>{status.label}</span>
                <strong>{status.value}</strong>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="thesis dark-section" aria-labelledby="thesis-title">
        <p className="section-index">The idea / 01</p>
        <div>
          <h2 id="thesis-title">
            Buzz captures the work. <em>Comb keeps the meaning.</em>
          </h2>
          <p>
            Buzz already gives people and agents one shared, signed history. Comb turns that history
            into living project memory: decisions, disagreements, storylines, gaps, owners, and next
            moves—each connected to the events that made it true.
          </p>
        </div>
      </section>

      <section className="proof-section" id="proof" aria-labelledby="proof-title">
        <header className="section-heading">
          <p className="section-index">The proof / 02</p>
          <div>
            <h2 id="proof-title">Ask why. Get the whole why.</h2>
            <p>
              A decision is only useful if you can inspect its evidence, dissent, blind spots, and what
              changed afterward.
            </p>
          </div>
        </header>
        <EvidenceExplorer />
      </section>

      <section className="mechanism" aria-labelledby="mechanism-title">
        <header>
          <p className="section-index">The mechanism / 03</p>
          <h2 id="mechanism-title">From room noise to organizational memory.</h2>
          <p>
            Comb does not declare truth from a summary. It proposes an understanding, shows its sources
            and blind spots, and lets people decide what becomes part of the project’s memory.
          </p>
        </header>
        <ol className="pipeline" aria-label="How activity becomes organizational memory">
          {pipeline.map((step, index) => (
            <li key={step}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <strong>{step}</strong>
            </li>
          ))}
        </ol>
      </section>

      <section className="capabilities" aria-labelledby="capabilities-title">
        <h2 className="visually-hidden" id="capabilities-title">
          What Comb makes possible
        </h2>
        {capabilityCards.map((card) => (
          <article key={card.number}>
            <span>{card.number}</span>
            <div>
              <h3>{card.title}</h3>
              <p>{card.body}</p>
            </div>
          </article>
        ))}
      </section>

      <section className="gap-section" aria-labelledby="gap-title">
        <header>
          <p className="section-index">The company can see itself / 04</p>
          <h2 id="gap-title">It notices who missed the memo.</h2>
          <p>
            Comb connects the ratified decision to the work still moving on old information, then asks
            before it acts.
          </p>
        </header>

        <article className="gap-card" aria-label="Illustrative communication gap">
          <div className="gap-card-head">
            <div>
              <span>Communication gap</span>
              <strong>Illustrative data</strong>
            </div>
            <b>17h</b>
          </div>
          <h3>Design is planning against the July 31 launch. Engineering ratified August 7.</h3>
          <ul>
            <li>Current design plan references the superseded July 31 decision.</li>
            <li>No member of Design viewed or reacted to the ratification thread.</li>
            <li>Design’s next scheduled milestone depends on the old date.</li>
          </ul>
          <div className="proposed-action">
            <span>Proposed action</span>
            <p>Post the ratified decision and changed dependencies to #design-atlas.</p>
            <strong>Requires human approval</strong>
          </div>
        </article>
      </section>

      <section className="buzz-native dark-section" aria-labelledby="buzz-title">
        <div>
          <p className="section-index">Built for Buzz / 05</p>
          <h2 id="buzz-title">Native to the room. Not another tab.</h2>
          <p>
            Comb is being built to read Buzz’s signed event stream, follow Buzz identities and room
            membership, and return proposed knowledge where people and agents can inspect, challenge,
            ratify, and use it.
          </p>
          <p className="honesty-note">
            Scoped compatibility proof passed against Buzz main at <code>acfbb1b</code> on July 22,
            2026. This validates the demo contract. It does not mean these semantics are upstreamed
            into Buzz.
          </p>
          <div className="compatibility-proof" aria-label="Buzz compatibility proof results">
            <header>
              <span>Buzz main proof</span>
              <strong>6/6 passed</strong>
            </header>
            <ul>
              <li>Stable self-attested coverage</li>
              <li>Outsider access denied</li>
              <li>Separate reviewer identity signed its own event</li>
              <li>Deleted source became unavailable</li>
              <li>Comb record invalidated and deleted</li>
              <li>Restart receipt remained idempotent</li>
            </ul>
            <a href={compatibilityProofUrl} target="_blank" rel="noreferrer">
              Inspect the proof <span aria-hidden="true">↗</span>
            </a>
          </div>
          <a className="text-link" href={buzzUrl} target="_blank" rel="noreferrer">
            Explore Buzz <span aria-hidden="true">↗</span>
          </a>
        </div>
        <ol className="primitive-grid" aria-label="Buzz primitives Comb is designed to use">
          {buzzPrimitives.map((primitive, index) => (
            <li key={primitive}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <strong>{primitive}</strong>
            </li>
          ))}
        </ol>
      </section>

      <section className="contribution-section" id="contributions" aria-labelledby="contribution-title">
        <header className="section-heading">
          <p className="section-index">Built in the open / 06</p>
          <div>
            <h2 id="contribution-title">Ship the destination. Upstream the primitives.</h2>
            <p>
              Comb is being developed independently against Buzz’s public repository. Nothing on this
              page is part of Buzz until Block’s maintainers review and merge it.
            </p>
          </div>
        </header>

        <div className="contribution-table" role="table" aria-label="Comb contribution status">
          <div className="contribution-row contribution-header" role="row">
            <span role="columnheader">Contribution</span>
            <span role="columnheader">Comb</span>
            <span role="columnheader">Buzz</span>
          </div>
          {contributionRows.map((row) => (
            <div className="contribution-row" role="row" key={row.name}>
              <strong role="cell">{row.name}</strong>
              <span role="cell" className={`table-status status-${row.comb.tone}`}>
                {row.comb.value}
              </span>
              <span role="cell" className={`table-status status-${row.buzz.tone}`}>
                {row.buzz.value}
              </span>
            </div>
          ))}
        </div>
        <p className="status-source">
          Status language is rendered from the project’s source-of-truth manifest. The scoped demo is
          tested. The event semantics are not upstreamed. “RFC open” is not “PR open.”
        </p>
        <a className="text-link" href={buzzRfcUrl} target="_blank" rel="noreferrer">
          Read Buzz RFC #2451 <span aria-hidden="true">↗</span>
        </a>
      </section>

      <section className="open-source" aria-labelledby="open-title">
        <p className="section-index">Open source / 07</p>
        <div>
          <h2 id="open-title">Fork the memory.</h2>
          <p>
            Run Comb with Buzz. Inspect every contract. Challenge the evidence model. Help build a
            workspace that gets smarter without pretending to know more than it does.
          </p>
          <div className="hero-actions">
            <a className="button button-primary" href={sourceUrl} target="_blank" rel="noreferrer">
              View the code <span aria-hidden="true">↗</span>
            </a>
            <a className="button button-secondary" href="#contributions">
              Read the contribution plan
            </a>
          </div>
        </div>
      </section>

      <section className="stride-teaser" aria-labelledby="stride-title">
        <p className="section-index">Comb came from a bigger question.</p>
        <div>
          <h2 id="stride-title">What if the whole workplace remembered?</h2>
          <p>
            Comb was developed while building STRIDE—an open operating system for how humans and agents
            talk, decide, make, and remember together. Comb brings that intelligence architecture to
            Buzz.
          </p>
          <a className="text-link" href={strideUrl} target="_blank" rel="noreferrer">
            Peek at STRIDE <span aria-hidden="true">→</span>
          </a>
          <span className="teaser-caption">Experimental / also built by That’s Cool</span>
        </div>
        <div className="stride-window" aria-hidden="true">
          <div className="window-bar">
            <i />
            <i />
            <i />
            <span>STRIDE / LIVING COMPANY OS</span>
          </div>
          <div className="window-body">
            <span className="window-kicker">The living company loop</span>
            <strong>Talk. Decide. Act. Make. Remember.</strong>
            <div className="window-steps">
              <span>Room</span>
              <span>Decision</span>
              <span>Artifact</span>
              <span>Memory</span>
            </div>
          </div>
        </div>
      </section>

      <section className="maker" aria-labelledby="maker-title">
        <p id="maker-title">Built by</p>
        <a href="https://thatscool.xyz" aria-label="That’s Cool" target="_blank" rel="noreferrer">
          <Image
            src="/thatscool-maker.png"
            alt="That’s Cool"
            width={1254}
            height={1254}
            sizes="(max-width: 680px) 75vw, 430px"
          />
        </a>
        <span>A little lab for ideas we can’t leave alone.</span>
      </section>

      <footer className="site-footer">
        <strong>Comb — independent open-source DLC for Buzz.</strong>
        <nav aria-label="Footer links">
          <a href={sourceUrl} target="_blank" rel="noreferrer">Source</a>
          <a href="#contributions">Contributions</a>
          <a href="https://www.apache.org/licenses/LICENSE-2.0" target="_blank" rel="noreferrer">License</a>
          <a href="https://thatscool.xyz" target="_blank" rel="noreferrer">That’s Cool</a>
          <a href={strideUrl} target="_blank" rel="noreferrer">STRIDE</a>
        </nav>
        <p>
          Buzz is an open-source project by Block, Inc. Comb is an independent project by That’s Cool
          and is not affiliated with, sponsored by, or endorsed by Block.
        </p>
        <span>© 2026 That’s Cool</span>
      </footer>
    </main>
  );
}
