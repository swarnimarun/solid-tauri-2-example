import { For, createEffect, createSignal } from "solid-js";
import { A } from "@solidjs/router";
import { FileInfo, commands } from "./bindings";
import { Button } from "./components/ui/button";

function Welcome() {
    const [recentlyUsedList, setRecentlyUsedList] = createSignal<FileInfo[]>([]);

    createEffect(async () => {
        let result = await commands.recentlyUsed();
        if (result.status === "ok")
            setRecentlyUsedList(result.data);
    });

    return (
        <div>
            <h1>Welcome to ZipTauri!</h1>
            <For each={recentlyUsedList()} fallback={<div></div>}>
                {(z) => <>{z.path}</>}
            </For>
            <A href="/upload">
                <Button> Goto Upload Page </Button>
            </A>
        </div>
    );
}

export default Welcome;