export function buildGithubPermalink({
  codebaseUrl,
  commitHash,
  filePath,
  startLine,
  endLine,
}: {
  codebaseUrl: string
  commitHash: string
  filePath: string
  startLine?: number
  endLine?: number
}) {
  const repositoryUrl = normalizeGithubRepositoryUrl(codebaseUrl)
  if (!repositoryUrl) {
    return null
  }

  const normalizedPath = filePath.replace(/^\/+/, '')
  const url = `${repositoryUrl}/blob/${commitHash}/${normalizedPath}`
  if (startLine === undefined) {
    return url
  }

  const fragment =
    endLine === undefined ? `#L${startLine}` : `#L${startLine}-L${endLine}`
  return `${url}${fragment}`
}

function normalizeGithubRepositoryUrl(codebaseUrl: string) {
  const sshMatch = codebaseUrl.match(
    /^git@github\.com:([^/]+)\/(.+?)(?:\.git)?$/,
  )
  if (sshMatch) {
    return `https://github.com/${sshMatch[1]}/${sshMatch[2]}`
  }

  try {
    const url = new URL(codebaseUrl)
    if (url.hostname !== 'github.com') {
      return null
    }

    const [owner, repo] = url.pathname.replace(/^\/+/, '').split('/')
    if (!owner || !repo) {
      return null
    }

    return `https://github.com/${owner}/${repo.replace(/\.git$/, '')}`
  } catch {
    return null
  }
}
