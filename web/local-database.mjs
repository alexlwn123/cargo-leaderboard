// Local PostgreSQL adapter with the same tagged-query interface as Neon HTTP.
// Used by integration tests and local development; hosted deployments use Neon.
import pg from 'pg';
export function postgres(url) {
  const pool = new pg.Pool({ connectionString: url, max: 10 });
  function statement(text, values) {
    return { text, values, then(resolve, reject) {
      return pool.query(text, values).then(result => result.rows).then(resolve, reject);
    } };
  }
  function sql(strings, ...values) {
    return statement(strings.reduce((text, part, i) => text + (i ? `$${i}` : '') + part, ''), values);
  }
  sql.query = statement;
  sql.transaction = async queries => {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');
      const results = [];
      for (const query of queries) results.push((await client.query(query.text, query.values)).rows);
      await client.query('COMMIT');
      return results;
    } catch (error) { await client.query('ROLLBACK'); throw error; }
    finally { client.release(); }
  };
  sql.end = () => pool.end();
  return sql;
}
