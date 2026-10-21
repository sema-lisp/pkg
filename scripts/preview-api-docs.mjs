// Copy verified public releases, or a local nested fixture, into a preview registry.
import { createHash, randomBytes } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const target = new URL(process.argv[2] ?? 'http://127.0.0.1:4177');
if (target.protocol !== 'http:' || !['127.0.0.1', 'localhost', '[::1]'].includes(target.hostname)) {
  throw new Error('The preview target must be a loopback HTTP server.');
}
const origin = 'https://pkg.sema-lang.com';
const nestedFixture = process.argv.includes('--nested-fixture');
const packages = nestedFixture ? ['nested-api-demo'] : ['sema-retry', 'sema-cache', 'sema-medley'];
async function checked(url, options) {
  const response = await fetch(url, { ...options, redirect: 'error' });
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response;
}

let token;
for (const name of packages) {
  const metadata = nestedFixture
    ? { package: { description: 'Local preview fixture with nested source, an imported alias, and a test suite' }, versions: [{ version: '0.1.0' }] }
    : await (await checked(`${origin}/api/v1/packages/${name}`)).json();
  const release = metadata.versions.find(item => !item.yanked && !item.tarball_url);
  if (!release) throw new Error(`${name}: no stored release is available`);
  const path = `/packages/${name}/${release.version}`;
  const existing = await fetch(new URL(path, target), { redirect: 'error' });
  if (existing.status === 200) {
    console.log(`${new URL(`${path}/api`, target)} (already present)`);
    continue;
  }
  if (existing.status !== 404) throw new Error(`Local registry: HTTP ${existing.status}`);
  const archive = nestedFixture
    ? execFileSync('tar', ['-czf', '-', '-C', fileURLToPath(new URL('../tests/fixtures/nested-api-demo/', import.meta.url)), '.'], { env: { ...process.env, COPYFILE_DISABLE: '1' } })
    : await (await checked(`${origin}/api/v1${path}/download`)).arrayBuffer();
  const digest = createHash('sha256').update(Buffer.from(archive)).digest('hex');
  if (!nestedFixture && digest !== release.checksum_sha256) throw new Error(`${name}: archive checksum mismatch`);
  if (!token) {
    const username = `preview-${randomBytes(6).toString('hex')}`;
    const registration = await checked(new URL('/api/v1/auth/register', target), {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username, email: `${username}@example.invalid`, password: randomBytes(24).toString('hex') }),
    });
    const cookie = registration.headers.get('set-cookie')?.match(/session=[^;]+/)?.[0];
    if (!cookie) throw new Error('Local registration did not return a session');
    const result = await (await checked(new URL('/api/v1/tokens', target), {
      method: 'POST', headers: { 'content-type': 'application/json', cookie },
      body: JSON.stringify({ name: 'API docs local preview' }),
    })).json();
    token = result.token;
  }
  const form = new FormData();
  form.set('metadata', JSON.stringify({
    description: metadata.package.description,
    repository_url: metadata.package.repository_url,
    sema_version_req: release.sema_version_req,
  }));
  form.set('tarball', new Blob([archive], { type: 'application/gzip' }), `${name}-${release.version}.tar.gz`);
  await checked(new URL(`/api/v1${path}`, target), {
    method: 'PUT', headers: { authorization: `Bearer ${token}` }, body: form,
  });
  console.log(`${new URL(`${path}/api`, target)} (${nestedFixture ? 'local fixture' : 'verified release'}: ${digest})`);
}
