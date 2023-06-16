/** @type {import('next').NextConfig} */
const nextConfig = {
  async rewrites() {
    return [
      {
        source: "/tweets",
        destination: "http://localhost:9090/tweets"
      }
    ]
  }
}

const productionConfig = {
  output: 'export'
}

module.exports = nextConfig
