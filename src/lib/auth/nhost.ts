import { createClient } from '@nhost/nhost-js';

const nhostSubdomain = (import.meta.env.VITE_NHOST_SUBDOMAIN as string | undefined)?.trim();
const nhostRegion = (import.meta.env.VITE_NHOST_REGION as string | undefined)?.trim();

const nhostAuthUrl = (import.meta.env.VITE_NHOST_AUTH_URL as string | undefined)?.trim();
const nhostStorageUrl = (import.meta.env.VITE_NHOST_STORAGE_URL as string | undefined)?.trim();
const nhostGraphqlUrl = (import.meta.env.VITE_NHOST_GRAPHQL_URL as string | undefined)?.trim();
const nhostFunctionsUrl = (import.meta.env.VITE_NHOST_FUNCTIONS_URL as string | undefined)?.trim();

const hasCloudConfig = Boolean(nhostSubdomain && nhostRegion);
const hasExplicitUrls = Boolean(
  nhostAuthUrl || nhostStorageUrl || nhostGraphqlUrl || nhostFunctionsUrl
);

export const authEnabled = hasCloudConfig || hasExplicitUrls;

export const nhost = authEnabled
  ? createClient({
      subdomain: nhostSubdomain,
      region: nhostRegion,
      authUrl: nhostAuthUrl,
      storageUrl: nhostStorageUrl,
      graphqlUrl: nhostGraphqlUrl,
      functionsUrl: nhostFunctionsUrl,
    })
  : null;

