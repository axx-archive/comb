export type StatusTone = "building" | "planned" | "tested" | "untested" | "unproposed";

export type StatusValue = {
  label: string;
  value: string;
  tone: StatusTone;
};

export const projectStatus = {
  combCore: {
    label: "Comb core",
    value: "Working / 29 tests",
    tone: "tested",
  },
  buzzCompatibility: {
    label: "Compatibility demo",
    value: "Tested / acfbb1b / Jul 22, 2026",
    tone: "tested",
  },
  upstream: {
    label: "Upstream",
    value: "Not yet proposed",
    tone: "unproposed",
  },
} satisfies Record<string, StatusValue>;

export const contributionRows = [
  {
    name: "Compatibility proof demo",
    comb: { value: "6/6 proof passed", tone: "tested" },
    buzz: { value: "Tested / acfbb1b", tone: "tested" },
  },
  {
    name: "Evidence-backed knowledge contracts",
    comb: { value: "Working in Comb", tone: "tested" },
    buzz: { value: "Not yet proposed", tone: "unproposed" },
  },
  {
    name: "Coverage + source manifests",
    comb: { value: "Working in Comb", tone: "tested" },
    buzz: { value: "Not yet proposed", tone: "unproposed" },
  },
  {
    name: "Ratification + supersession",
    comb: { value: "Working in Comb", tone: "tested" },
    buzz: { value: "Not yet proposed", tone: "unproposed" },
  },
  {
    name: "Huddle transcript publication",
    comb: { value: "Planned", tone: "planned" },
    buzz: { value: "Not yet proposed", tone: "unproposed" },
  },
  {
    name: "Permission-loss propagation",
    comb: { value: "Source deletion tested", tone: "tested" },
    buzz: { value: "Not yet proposed", tone: "unproposed" },
  },
] satisfies Array<{
  name: string;
  comb: { value: string; tone: StatusTone };
  buzz: { value: string; tone: StatusTone };
}>;
