// The plan's picture library: logos of the parties (client, our company, vendors, sub-vendors) and
// other images, kept in the plan file and used in print headers, dashboards and sheets.
import { For, Show } from "solid-js";
import { askYes } from "./ui";
import { Picture } from "./api";
import { act, call, P, openDialog, openMenu, Item, toast } from "./store";
import { Dialog } from "./ui";

export const ROLES = ["Client", "Our company", "Main contractor", "Consultant", "Vendor", "Sub-vendor", "Partner", "Other"];
export const pictures = () => (P()?.pictures ?? []) as Picture[];
export const picData = (id: number) => pictures().find(x => x.id === id)?.data ?? "";
const label = (x: Picture) => `${x.name || "Picture"}${x.role ? ` (${x.role})` : ""}`;

/** Asks for a picture file and adds it to the library; resolves to its id. */
export const addPicture = async (role = "Client", name = ""): Promise<number | null> => {
  const data = await call<string | null>("pick_logo");
  if (!data) return null;
  const before = new Set(pictures().map(x => x.id));
  if (!(await act({ Picture: { id: 0, pic: { id: 0, name: name || role, role, data } } }))) return null;
  return pictures().find(x => !before.has(x.id))?.id ?? null;
};

/** A menu of the library's pictures (and "Add a picture…") that hands the chosen id to `use`. */
export const pictureMenu = (e: MouseEvent, use: (id: number) => void) => openMenu(e, [
  ...pictures().map((x): Item => ({ label: label(x), run: () => use(x.id) })),
  ...(pictures().length ? ["-" as const] : []),
  { label: "Add a picture to the library…", run: () => addPicture().then(id => id && use(id)) },
  { label: "Manage pictures…", run: pictureLibrary },
]);

/** The library: each picture with its role and name; add, rename, change the role, remove. */
export const Library = () => (
  <div class="piclib">
    <For each={pictures()} fallback={<p class="muted">No pictures yet. Add the logos of the client, your company, vendors and sub-vendors once; then use them in printouts, dashboards and sheets.</p>}>{x => (
      <div class="picrow">
        <div class="thumb"><img src={x.data} alt="" /></div>
        <select title="Whose logo or what picture this is" onChange={e => act({ Picture: { id: x.id, pic: { ...x, role: e.currentTarget.value } } })}>
          <For each={ROLES.includes(x.role) || !x.role ? ROLES : [x.role, ...ROLES]}>{r => <option selected={r === x.role}>{r}</option>}</For></select>
        <input value={x.name} placeholder="Name, e.g. the company" onChange={e => act({ Picture: { id: x.id, pic: { ...x, name: e.currentTarget.value } } })} />
        <button class="btn sm" title="Remove this picture from the plan (also from sheets)" onClick={async () => { if (await askYes("Remove Picture", `Remove ${label(x)} from this plan? It is also taken off the sheets that use it.`, "Remove")) act({ Picture: { id: x.id, pic: null } }); }}>×</button>
      </div>)}</For>
    <div class="inline"><For each={["Client", "Our company", "Vendor", "Sub-vendor"]}>{r => <button class="btn sm" onClick={() => addPicture(r).then(id => id && toast(`${r} logo added to the plan.`, true))}>+ {r} logo…</button>}</For>
      <button class="btn sm" onClick={() => addPicture("Other", "Picture")}>+ Other picture…</button></div>
    <Show when={pictures().length}><p class="muted">Pictures are saved inside the plan file, so they travel with it.</p></Show>
  </div>
);

/** Project › Pictures: the library in a dialog. */
export const pictureLibrary = () => openDialog(() => <Dialog title="Pictures and logos" wide help="pictures"><Library /></Dialog>);
