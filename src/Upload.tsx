import { For, createEffect, createSignal } from "solid-js";
import { commands } from "./bindings";

import { listen, Event as TauriEvent } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from "./components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./components/ui/dialog";
import { TextField, TextFieldRoot } from "./components/ui/textfield";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "./components/ui/card";

function RecursiveRender(props: { paths: string[] }) {
  return <ul class="flex-col space-y-1">
    <For each={props.paths} fallback={<></>}>
      {(e) => <li><Button disabled variant="secondary">{e}</Button></li>}
    </For>
  </ul>
}

// handle graphs
interface TauriPayload {
  path: string,
}

function App() {
  const [pathMap, setPathMap] = createSignal<string[]>([], { equals: false });
  const [alert, showAlert] = createSignal(false);
  const [password, setPassword] = createSignal("");

  async function getZip() {
    let file = await open({ title: "Open zip", directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (file) {
      // clear up path map
      setPathMap([]);
      await commands.tryUnzip(file);
    }
  }

  createEffect(() => {
    listen("file-password-request", async (_) => {
      // create a alert & prompt for password
      showAlert(true);
    });

    listen("unzip-file", async (event: TauriEvent<TauriPayload>) => {
      // add file to pathMap
      setPathMap((e) => {
        e.push(event.payload.path);
        return e;
      });
    });

    // note: ensure all the listeners have been registered first.
    let path = localStorage.getItem("unzip-path");
    // clear local storage so that we don't get stuck
    localStorage.removeItem("unzip-path");
    if (path) {
      // clear up path map
      setPathMap([]);
      commands.tryUnzip({ path });
    }
  });

  async function filePasswordSubmit(password: string) {
    await commands.filePasswordSubmit(password);
  }

  async function passwordDialogHandle(b: boolean) {
    if (!b) {
      // raise an error and cancel unzip request
      await commands.cancelUnzip();
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
            // don't log password
            // console.log("password: ", password());
            await filePasswordSubmit(password());
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
          {<RecursiveRender paths={pathMap()} />}
        </CardContent>
        <CardFooter class="flex justify-center">
          <Button onClick={() => getZip()}> . open file . </Button>
        </CardFooter>
      </Card>
    </div>
  );
}

export default App;
