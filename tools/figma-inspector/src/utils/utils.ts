
export function writeTextToClipboard(str: string) {
    const prevActive = document.activeElement;
    const textArea = document.createElement("textarea");

    textArea.value = str;

    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";

    document.body.appendChild(textArea);

    textArea.focus();
    textArea.select();

    return new Promise<void>((res, rej) => {
        document.execCommand("copy") ? res() : rej();
        textArea.remove();

        if (prevActive && prevActive instanceof HTMLElement) {
            prevActive.focus();
        }
    });
}

export function copyToClipboard(slintProperties: string) {
    writeTextToClipboard(slintProperties);
    dispatchTS("copyToClipboard", {
        result: true,
    });
}
