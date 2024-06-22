import { For, createSignal } from "solid-js";
import { FileInfo, commands } from "./bindings";

import { open } from '@tauri-apps/plugin-dialog';


function App() {
  const [zipList, setZipList] = createSignal<FileInfo[]>([]);

  async function getZip() {
    let file = await open({ title: "Open zip", directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (!file) return;
    let result = await commands.tryUnzip(file);
    if (result.status === "ok")
      setZipList(result.data);
  }

  return (
    <div>
      <ul>
        <For each={zipList()} fallback={<div></div>}>
          {(z) => <li>{z.path}</li>}
        </For>
      </ul>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          getZip();
        }}
      >
        <button
          type="submit"
        >
          open
        </button>
      </form>
    </div>
  );
}

export default App;
