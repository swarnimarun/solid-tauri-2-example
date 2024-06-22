import { For, createEffect, createSignal } from "solid-js";
import { FileInfo, commands } from "./bindings";

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
            <button onClick={() => { location.href = "/upload" }}>
                upload
            </button>
        </div>
    );
}

export default Welcome;