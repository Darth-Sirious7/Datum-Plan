// The tutorial notes: a plan can carry one note per view (project.ui.tour); the note of the view
// shown sits at the bottom right, with Back and Next through the sidebar's views.
import { For, Show, createSignal } from "solid-js";
import { Out } from "./api";
import { out, view, View, call, setOut, setBackstage, toast } from "./store";
import { C } from "./cmds";

const KEY = "datum.tourOff";
const read = () => { try { return localStorage.getItem(KEY) === "1"; } catch { return false; } };
export const [tourOff, setTourOffRaw] = createSignal(read());
export const setTourOff = (v: boolean) => { setTourOffRaw(v); try { localStorage.setItem(KEY, v ? "1" : "0"); } catch { /* private window */ } };
const notes = () => (out()?.project.ui?.tour ?? null) as Record<string, string> | null;
export const hasTour = () => !!notes();

/** File › New › Tutorial: opens the guided example in a new tab, notes on. */
export const tutorial = async () => {
  const o = await call<Out>("tutorial_example");
  if (!o) return;
  setOut(o); setTourOff(false); setBackstage(null); C.view("setup");
  toast("The tutorial plan is open. Follow the note at the bottom right through every view.", true);
};

export const TourCard = (p: { views: [View, string][] }) => {
  const list = () => p.views.filter(([k]) => notes()?.[k]);
  const at = () => list().findIndex(([k]) => k === view());
  const go = (d: number) => { const l = list(), k = at(); const n = l[(k < 0 ? 0 : k + d + l.length) % l.length]; if (n) C.view(n[0]); };
  const text = () => notes()?.[view()] ?? "";
  return (
    <Show when={!tourOff() && text()}>
      <aside class="tour noprint" role="note" aria-label="Tutorial note">
        <header><b>Tutorial · {list()[at()]?.[1] ?? "This view"}</b><span class="muted">{at() + 1} of {list().length}</span><button class="x" title="Hide the tutorial notes (Help › Tutorial Notes shows them again)" onClick={() => setTourOff(true)}>×</button></header>
        <div class="tbody"><For each={text().split("\n\n")}>{para => para.startsWith("• ")
          ? <ul><For each={para.split("\n")}>{li => <li>{li.replace(/^• /, "")}</li>}</For></ul>
          : <p>{para}</p>}</For></div>
        <footer><button class="btn sm" onClick={() => go(-1)}>‹ Back</button><span class="grow" /><button class="btn sm pri" onClick={() => go(1)}>{at() === list().length - 1 ? "Start again" : "Next ›"}</button></footer>
      </aside>
    </Show>
  );
};
