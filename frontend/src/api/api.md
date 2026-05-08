<!-- @format -->

1. Nexus Mods REST API v1 — api.nexusmods.com/v1
   The classic API. Requires a personal API key. The endpoints you actually care about:
   EndpointWhat it gives youNotesGET /games/stardewvalley/mods/{id}.jsonversion, name, author, summary, updated_timestampYour primary update check sourceGET /games/stardewvalley/mods/{id}/files.jsonAll uploaded file versions with category_name (MAIN, UPDATE, etc.)Needed before downloadingGET /games/stardewvalley/mods/{id}/files/{file_id}/download_link.jsonCDN download URLPremium only — free users get 403GET /users/validate.jsonUser info including is_premium flagCheck this 2. SMAPI Web API — smapi.io/api/v3.0/mods
   This is the hidden gem for your use case. It has one /mods endpoint that crossreferences mods against a variety of sources (the wiki, Chucklefish, CurseForge, ModDrop, and Nexus) to provide metadata mainly intended for update checks. Modding.wiki
   You POST a batch of mod IDs and get back update info for all of them at once. This is exactly what SMAPI itself uses internally. Extra metadata fields are available if you set includeExtendedMetadata: true, including the Nexus mod ID, the normalized mod name, GitHub repo, and custom mod page URL. Modding.wiki
   This is better than hitting the Nexus API directly for update checks because:

No API key required
Batch request — one call for all your mods
Normalizes versions across sources
Cross-references the SMAPI compatibility wiki automatically

Example request body:
json{
"mods": [
{ "id": "Pathoschild.Automate", "updateKeys": ["Nexus:1066"] },
{ "id": "CJBok.CheatsMenu", "updateKeys": ["Nexus:4"] }
],
"includeExtendedMetadata": true
}

On Scan:

1. Read all manifest.json files locally
   → Build a map of UniqueID → InstalledMod

2. POST to smapi.io/api/v3.0/mods with all UniqueIDs + UpdateKeys
   → Get back latest versions for all mods in one request (free, no key)

3. Compare installed vs latest versions
   → Flag UpdateAvailable

4. Check Dependencies locally
   → For each mod's Dependencies array, check if UniqueID exists in your installed map
   → Flag MissingDependency or OutdatedDependency

On "Install Update" click (for update OR missing dependency): 5. Hit Nexus v1 API to get the file list + download URL
→ Premium: stream download directly
→ Free: open browser to mod page

This means you only need a Nexus API key for downloading — the update checking itself works free via SMAPI's API. That's a much better UX since users can see update status without setting up a key first.
The one gap worth noting: if a required dependency isn't installed at all (not just outdated), you know its UniqueID from the manifest but you don't automatically know its Nexus mod ID to send the user to the right download page. The includeExtendedMetadata response from the SMAPI API includes the Nexus ID for known mods, so you can look it up there. For unknown mods you'd fall back to a search URL.
