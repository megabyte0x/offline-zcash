// Serves the static homepage and sends every other hostname
// (www., offlinezec.com) to the primary domain.
const PRIMARY = "offlinezcash.com";

export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    if (url.hostname !== PRIMARY) {
      url.protocol = "https:"; url.hostname = PRIMARY; url.port = "";
      return Response.redirect(url.toString(), 301);
    }
    const res = await env.ASSETS.fetch(req);
    const h = new Headers(res.headers);
    h.set("X-Content-Type-Options", "nosniff");
    h.set("Referrer-Policy", "strict-origin-when-cross-origin");
    if (url.pathname.startsWith("/img/")) h.set("Cache-Control", "public, max-age=604800");
    return new Response(res.body, { status: res.status, headers: h });
  }
};
