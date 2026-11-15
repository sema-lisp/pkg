import type { APIRequestContext } from '@playwright/test';
import { gzipSync } from 'node:zlib';

const BASE = 'http://localhost:3111';

/** Register a user and return the session cookie string. */
export async function register(
  request: APIRequestContext,
  username: string,
  email: string,
  password = 'password123',
): Promise<string> {
  const res = await request.post(`${BASE}/api/v1/auth/register`, {
    data: { username, email, password },
  });
  if (!res.ok()) throw new Error(`Register failed: ${res.status()} ${await res.text()}`);
  const setCookie = res.headers()['set-cookie'] ?? '';
  const match = setCookie.match(/session=([^;]+)/);
  if (!match) throw new Error('No session cookie in register response');
  return match[1];
}

/** Login and return the session cookie string. */
export async function login(
  request: APIRequestContext,
  username: string,
  password = 'password123',
): Promise<string> {
  const res = await request.post(`${BASE}/api/v1/auth/login`, {
    data: { username, password },
  });
  if (!res.ok()) throw new Error(`Login failed: ${res.status()} ${await res.text()}`);
  const setCookie = res.headers()['set-cookie'] ?? '';
  const match = setCookie.match(/session=([^;]+)/);
  if (!match) throw new Error('No session cookie in login response');
  return match[1];
}

/** Create an API token and return the raw token string. */
export async function createToken(
  request: APIRequestContext,
  session: string,
  name = 'test-token',
): Promise<string> {
  const res = await request.post(`${BASE}/api/v1/tokens`, {
    data: { name },
    headers: { cookie: `session=${session}` },
  });
  if (!res.ok()) throw new Error(`Token creation failed: ${res.status()}`);
  const body = await res.json();
  return body.token;
}

/** Build the small valid tar.gz fixture accepted by the publish endpoint. */
function packageTarball(): Buffer {
  const content = Buffer.from('(module test)\n');
  const header = Buffer.alloc(512);
  header.write('package.sema');
  header.write('0000644\0', 100);
  header.write('0000000\0', 108);
  header.write('0000000\0', 116);
  header.write(content.length.toString(8).padStart(11, '0') + '\0', 124);
  header.write('00000000000\0', 136);
  header.fill(0x20, 148, 156);
  header[156] = '0'.charCodeAt(0);
  header.write('ustar\0', 257);
  header.write('00', 263);
  let checksum = 0;
  for (const byte of header) checksum += byte;
  header.write(checksum.toString(8).padStart(6, '0') + '\0 ', 148);
  const padding = Buffer.alloc((512 - (content.length % 512)) % 512);
  return gzipSync(Buffer.concat([header, content, padding, Buffer.alloc(1024)]));
}

/** Publish a package with a valid tarball via multipart. */
export async function publishPackage(
  request: APIRequestContext,
  token: string,
  name: string,
  version: string,
  description = `Test package: ${name}`,
): Promise<void> {
  const boundary = '----e2etestboundary';
  const meta = JSON.stringify({ description });
  const tarball = packageTarball();

  let body = '';
  body += `--${boundary}\r\n`;
  body += 'Content-Disposition: form-data; name="metadata"\r\n\r\n';
  body += meta + '\r\n';
  body += `--${boundary}\r\n`;
  body += 'Content-Disposition: form-data; name="tarball"; filename="pkg.tar.gz"\r\n';
  body += 'Content-Type: application/gzip\r\n\r\n';

  const parts = [
    Buffer.from(body),
    tarball,
    Buffer.from(`\r\n--${boundary}--\r\n`),
  ];
  const fullBody = Buffer.concat(parts);

  const res = await request.put(`${BASE}/api/v1/packages/${name}/${version}`, {
    data: fullBody,
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': `multipart/form-data; boundary=${boundary}`,
    },
  });
  if (!res.ok()) {
    const text = await res.text();
    throw new Error(`Publish failed: ${res.status()} ${text}`);
  }
}

/** Yank a version. */
export async function yankVersion(
  request: APIRequestContext,
  token: string,
  name: string,
  version: string,
): Promise<void> {
  const res = await request.post(`${BASE}/api/v1/packages/${name}/${version}/yank`, {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!res.ok()) throw new Error(`Yank failed: ${res.status()}`);
}

/** Add an owner to a package. */
export async function addOwner(
  request: APIRequestContext,
  token: string,
  packageName: string,
  username: string,
): Promise<void> {
  const res = await request.put(`${BASE}/api/v1/packages/${packageName}/owners`, {
    data: { username },
    headers: { authorization: `Bearer ${token}` },
  });
  if (!res.ok()) throw new Error(`Add owner failed: ${res.status()}`);
}

/** Remove an owner from a package. */
export async function removeOwner(
  request: APIRequestContext,
  token: string,
  packageName: string,
  username: string,
): Promise<void> {
  const res = await request.delete(`${BASE}/api/v1/packages/${packageName}/owners`, {
    data: { username },
    headers: { authorization: `Bearer ${token}` },
  });
  if (!res.ok()) throw new Error(`Remove owner failed: ${res.status()}`);
}

/** Set a session cookie on a browser page. */
export async function setSession(
  context: { addCookies: (cookies: Array<{ name: string; value: string; url: string }>) => Promise<void> },
  session: string,
): Promise<void> {
  await context.addCookies([{ name: 'session', value: session, url: BASE }]);
}
