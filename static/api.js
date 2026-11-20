// Shared helpers for all pages. Every API error has the body {"error": "..."}.

const API = {
    async request(method, path, body) {
        const options = { method, headers: {} };
        if (body !== undefined) {
            options.headers['Content-Type'] = 'application/json';
            options.body = JSON.stringify(body);
        }
        const response = await fetch(`/api${path}`, options);
        let data = null;
        const text = await response.text();
        if (text) {
            try {
                data = JSON.parse(text);
            } catch {
                data = text;
            }
        }
        if (!response.ok) {
            const message = (data && data.error) || response.statusText || `HTTP ${response.status}`;
            throw new Error(message);
        }
        return data;
    },

    get: path => API.request('GET', path),
    post: (path, body) => API.request('POST', path, body),
    put: (path, body) => API.request('PUT', path, body),
    del: path => API.request('DELETE', path),

    /** Downloads a file from the API, using the server's file name if it sends one. */
    async download(path, fallbackName) {
        const response = await fetch(`/api${path}`);
        if (!response.ok) {
            let message = response.statusText;
            try {
                message = (await response.json()).error || message;
            } catch { /* not JSON */ }
            throw new Error(message);
        }
        const blob = await response.blob();
        const match = (response.headers.get('Content-Disposition') || '').match(/filename="(.+)"/);
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = match ? match[1] : fallbackName;
        document.body.appendChild(a);
        a.click();
        URL.revokeObjectURL(a.href);
        a.remove();
    },
};

/** Formats a number with one decimal, or "--" for missing sensor values. */
function fmt(value, digits = 1) {
    return (value === null || value === undefined) ? '--' : Number(value).toFixed(digits);
}

/**
 * Shows a short status message in `element` (kind: "ok" or "error").
 * Success messages disappear after a few seconds.
 */
function showMessage(element, text, kind = 'ok') {
    element.textContent = text;
    element.className = `message ${kind}`;
    clearTimeout(element._timer);
    if (kind === 'ok') {
        element._timer = setTimeout(() => { element.textContent = ''; element.className = 'message'; }, 4000);
    }
}

/** Approximate screen colour of an RGBWW value (for previews only). */
function previewColor({ r, g, b, ww, cw }) {
    let red = r + ww * 0.8 + cw * 0.6;
    let green = g + ww * 0.6 + cw * 0.7;
    let blue = b + ww * 0.2 + cw * 0.9;
    // Scale all channels together instead of clipping each, so the hue stays visible
    const max = Math.max(red, green, blue);
    if (max > 255) {
        red = red * 255 / max;
        green = green * 255 / max;
        blue = blue * 255 / max;
    }
    return `rgb(${Math.round(red)}, ${Math.round(green)}, ${Math.round(blue)})`;
}
