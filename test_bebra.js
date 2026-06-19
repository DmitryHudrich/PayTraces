const BASE_URL = 'http://localhost:3000';
const API_VERSION = '1';

async function request(path, { method = 'GET', body, params } = {}) {
  const url = new URL(path, BASE_URL);
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined) url.searchParams.set(key, value);
    }
  }

  const res = await fetch(url, {
    method,
    headers: {
      'X-API-Version': API_VERSION,
      'Content-Type': 'application/json'
    },
    body: body ? JSON.stringify(body) : undefined
  });

  const data = await res.json().catch(() => null);

  if (!res.ok) {
    throw new Error(`${method} ${path} -> ${res.status}: ${JSON.stringify(data)}`);
  }

  return data;
}

async function main() {
  const address = process.argv[2];
  const chainId = process.argv[3] ? Number(process.argv[3]) : 1;

  if (!address) {
    console.error('usage: node script.js <address> [chain_id]');
    process.exit(1);
  }

  const job = await request('/jobs/ingest', {
    method: 'POST',
    body: { address, chain_id: chainId, max_depth: 2, max_nodes: 200, from_block: 25278225 }
  });
  console.log('ingest job:', job);

  const status = await request(`/jobs/${job.job_id}`);
  console.log('job status:', status);

  const graph = await request('/graph', {
    params: { address, chain_id: chainId, max_depth: 2, page: 0, page_size: 100 }
  });
  console.log('graph page 0:', graph);

  const score = await request('/score', { params: { address, chain_id: chainId } });
  console.log('score:', score);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
