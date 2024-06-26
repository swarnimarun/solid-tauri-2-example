import { For, createEffect, createSignal } from "solid-js";
import { FileInfo, commands } from "./bindings";

import { listen, Event as TauriEvent } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from "./components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./components/ui/dialog";
import { TextField, TextFieldLabel, TextFieldRoot } from "./components/ui/textfield";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "./components/ui/card";

interface PathMap {
  inner: Map<string, PathMap[]>,
}

function Keys(map: PathMap | null): string[] {
  if (!map) return [];
  return [...map.inner.keys()];
}

function Values(map: PathMap | null, path: string): PathMap[] | undefined {
  if (!map) return [];
  return map.inner.get(path);
}

function RecursiveRender(props: { map: PathMap | null }) {
  return <div>
    <For each={Keys(props.map)} fallback={<></>}>
      {
        (e) =>
          <ul>
            <h2>{e}</h2>
            <br />
            <For each={Values(props.map, e) || []}>
              {(e) => <li><RecursiveRender map={e} /></li>}
            </For>
          </ul>
      }
    </For>
  </div>;
}
function appendToPathMap(v: PathMap, path: string[]): PathMap {
  let map = v;
  for (let i = 0; i < path.length; i++) {
    if (!map.inner.has(path[i])) {
      map.inner.set(path[i], []);
    }
    // map = map.inner.get(path[i])!;
  }
  return map;
}
function mapFromPath(path: string[]): PathMap {
  let map = new Map<string, PathMap[]>();
  map.set(path[0], []);
  return {
    inner: map
  };
}

interface TauriPayload {
  path: string[],
}

function App() {
  const [pathMap, setPathMap] = createSignal<PathMap | null>(null);
  const [alert, showAlert] = createSignal(false);
  const [password, setPassword] = createSignal("");

  async function getZip() {
    let file = await open({ title: "Open zip", directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (file)
      commands.tryUnzip(file);
  }

  createEffect(() => {
    listen("file-password-request", async (_) => {
      // create a alert & prompt for password
      showAlert(true);
    });

    listen("unzip-file", async (event: TauriEvent<TauriPayload>) => {
      let payload: TauriPayload = event.payload;
      // add file to pathMap
      setPathMap((v) => {
        if (!v)
          return mapFromPath(payload.path);
        appendToPathMap(v, payload.path);
        return v;
      });
    });
  });

  async function filePasswordSubmit(password: string) {
    await commands.filePasswordSubmit(password);
  }

  async function passwordDialogHandle(b: boolean) {
    if (!b) {
      // raise an error and cancel unzip request
    }
    // close alert
    showAlert(b);
  }

  return (
    <div class="flex h-screen dark justify-center items-center">
      <Dialog open={alert()} onOpenChange={passwordDialogHandle}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Password required</DialogTitle>
            <DialogDescription>for unzipping! :3</DialogDescription>
          </DialogHeader>
          <form onSubmit={async (e) => {
            e.preventDefault();
            console.log("password: ", password());
            // filePasswordSubmit(password())
            // close alert
            showAlert(false);
            // reset password
            setPassword("");
          }}>
            <TextFieldRoot>
              <TextField type="password" placeholder="Password" onChange={(e: Event & { currentTarget: HTMLInputElement }) => setPassword(e.currentTarget?.value)} />
            </TextFieldRoot>
            <br />
            <DialogFooter class="w-full">
              <Button variant={"outline"} onClick={() => showAlert(false)}>Cancel</Button>
              <Button type="submit" variant={"default"} >Submit</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Card class="min-w-[600px] flex-col items-center justify-evenly">
        <CardHeader class="text-center">
          <CardTitle>Explorer</CardTitle>
          <CardDescription>zipped file explorer.</CardDescription>
        </CardHeader>
        <CardContent class="overflow-y-scroll max-h-96 min-h-48">
          {<RecursiveRender map={pathMap()} />}
        </CardContent>
        <CardFooter class="flex justify-center">
          <Button onClick={() => getZip()}> . open file . </Button>
        </CardFooter>
      </Card>
    </div>
  );
}

export default App;
