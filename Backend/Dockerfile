# Stage 1: Build
FROM node:20-alpine AS builder

WORKDIR /app

# Copy lockfile and manifest first so Docker can cache the install layer.
# Both package.json AND package-lock.json must be present; npm ci enforces
# the lockfile for a bit-for-bit reproducible install.
COPY package.json package-lock.json ./
COPY prisma ./prisma/

# Clean install from lockfile — fails if lockfile is out of sync with package.json
RUN npm ci

# Generate Prisma client
RUN npx prisma generate

# Copy source and build
COPY . .
RUN npm run build

# Stage 2: Production image
FROM node:20-alpine AS production

WORKDIR /app

# Ensures config validates strictly and refuses to start with missing or
# default secrets/URLs — real values must be injected at deploy time.
ENV NODE_ENV=production

# Copy manifest + lockfile so npm ci can reproduce the exact dep tree
COPY package.json package-lock.json ./

# --omit=dev installs only production dependencies (replaces deprecated --only=production)
RUN npm ci --omit=dev

# Copy Prisma schema and the generated client built in stage 1
COPY prisma ./prisma/
COPY --from=builder /app/node_modules/.prisma ./node_modules/.prisma

# Copy compiled application
COPY --from=builder /app/dist ./dist

EXPOSE 3000

# Support both API mode (default) and worker mode via APP_MODE env var
CMD ["sh", "-c", "if [ \"$APP_MODE\" = \"worker\" ]; then node dist/server.js --worker; else node dist/server.js; fi"]
