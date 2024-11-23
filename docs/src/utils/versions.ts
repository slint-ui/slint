export interface Version {
    version: string;
    name?: string;
    url: string;
    preferred?: boolean;
}

let versionsCache: Version[] | null = null;

export async function getVersions(): Promise<Version[]> {
    if (versionsCache) {
        return versionsCache;
    }

    try {
        const response = await fetch(
            "https://releases.slint.dev/versions.json",
        );
        const versions: Version[] = await response.json();
        versionsCache = versions;
        return versions;
    } catch (error) {
        console.error("Failed to fetch versions:", error);
        return [];
    }
}