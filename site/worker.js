// Serves the static homepage and sends every other hostname
// (www., offlinezec.com) to the primary domain.
const PRIMARY = "offlinezcash.com";
const CATALOGUE = "https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json";

export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    if (url.hostname !== PRIMARY) {
      url.protocol = "https:"; url.hostname = PRIMARY; url.port = "";
      return Response.redirect(url.toString(), 301);
    }
    // Short catalogue URL for Basecamp → Settings → Package Repositories.
    // Basecamp's downloader follows redirects (curl FOLLOWLOCATION, https only).
    if (url.pathname === "/basecamp" || url.pathname === "/basecamp/") {
      return Response.redirect(CATALOGUE, 302);
    }
    if (url.pathname === "/pitch" || url.pathname === "/pitch/") {
      return Response.redirect("https://" + PRIMARY + "/basecamp-pitch", 301);
    }
    const res = await env.ASSETS.fetch(req);
    const h = new Headers(res.headers);
    h.set("X-Content-Type-Options", "nosniff");
    h.set("Referrer-Policy", "strict-origin-when-cross-origin");
    if (/^\/(img|og)\//.test(url.pathname) || /\.(png|ico)$/.test(url.pathname)) h.set("Cache-Control", "public, max-age=604800");
    return new Response(res.body, { status: res.status, headers: h });
  }
};
