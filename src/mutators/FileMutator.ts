import type { TreePath } from "../messages/TreePath";
import { trigger } from "../ipc";

export default class FileMutator {
    #path: TreePath;
    #workspaceRoot: string;

    constructor(path: TreePath, workspaceRoot: string = "") {
        this.#path = path;
        this.#workspaceRoot = workspaceRoot;
    }

    normalizePath(path: string): string {
        return path.replace(/\\/g, "/");
    }

    buildFullPath(): string {
        if (!this.#workspaceRoot) {
            return this.normalizePath(this.#path.repo_path);
        }
        const cleanRoot = this.normalizePath(this.#workspaceRoot).replace(/\/$/, "");
        const cleanRepoPath = this.normalizePath(this.#path.repo_path).replace(/^\//, "");
        return `${cleanRoot}/${cleanRepoPath}`;
    }

    handle(event: string | undefined) {
        if (!event) {
            return;
        }

        switch (event) {
            case "open-with-default-app":
                this.onOpenWithDefaultApp();
                break;
            case "open-in-explorer":
                this.onOpenInExplorer();
                break;
            case "copy-full-path":
                this.onCopyFullPath();
                break;
            case "copy-relative-path":
                this.onCopyRelativePath();
                break;
            default:
                console.log(`unimplemented file operation '${event}'`, this);
        }
    }

    onOpenWithDefaultApp() {
        trigger("open_file_with_default_app", { file_path: this.buildFullPath() });
    }

    onOpenInExplorer() {
        trigger("open_file_in_explorer", { file_path: this.buildFullPath() });
    }

    onCopyFullPath() {
        navigator.clipboard.writeText(this.buildFullPath()).catch((err) => {
            console.error("Failed to copy full path to clipboard:", err);
        });
    }

    onCopyRelativePath() {
        const relativePath = this.normalizePath(this.#path.relative_path || this.#path.repo_path);
        navigator.clipboard.writeText(relativePath).catch((err) => {
            console.error("Failed to copy relative path to clipboard:", err);
        });
    }
}
