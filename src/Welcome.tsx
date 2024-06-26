import { For, createEffect, createSignal } from "solid-js";
import { A } from "@solidjs/router";
import { FileInfo, commands } from "./bindings";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "./components/ui/card";

function Welcome() {
    const [recentlyUsedList, setRecentlyUsedList] = createSignal<FileInfo[]>([]);

    createEffect(async () => {
        let result = await commands.recentlyUsed();
        if (result.status === "ok")
            setRecentlyUsedList(result.data);
    });

    return (
        <div class="flex h-screen dark justify-center items-center">
            <Card>
                <CardHeader>
                    <CardTitle>Welcome to ZipTauri!</CardTitle>
                    <CardDescription>Open recent files</CardDescription>
                </CardHeader>
                <CardContent class="flex flex-row justify-center">
                    <ul>
                        <For each={recentlyUsedList()} fallback={<Button variant="secondary">No files opened yet!</Button>}>
                            {
                                (z) => <li>
                                    <A href="/upload" onClick={() => localStorage.setItem("unzip-path", z.path)}>
                                        <Button variant="outline">{z.path}</Button>
                                    </A>
                                </li>
                            }
                        </For>
                    </ul>
                </CardContent>
                <CardFooter class="flex justify-center">
                    <A href="/upload">
                        <Button>New File</Button>
                    </A>
                </CardFooter>
            </Card>
        </div >
    );
}

export default Welcome;