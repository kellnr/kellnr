<template>
  <v-container fluid class="pa-4">
    <v-card elevation="0" class="docs-card" data-testid="publish-docs">
      <SectionHeader icon="mdi-book-open-page-variant" title="Publish Crate Documentation" />

      <v-card-text class="docs-content">
        <p>
          If enabled, <a href="https://doc.rust-lang.org/rustdoc/index.html">Rustdoc</a> documentation gets auto-generated for each uploaded <i>crate</i>.
          If you just uploaded a new <i>crate</i> and no documentation is available, the <i>crate</i> is still in the queue and will be processed soon. For more information about the documentation features of <i>Kellnr</i> see the official <a href="https://kellnr.io/documentation">documentation</a>.
          If for some reason, the documentation generation fails or a custom documentation should be used, see below for instruction for manual documentation uploads.
        </p>

        <SubsectionHeader icon="mdi-upload" title="Manual Documentation Publishing" class="mt-6" />
        <p>
          <i>Kellnr</i> provides an easy option to host the corresponding
          <a href="https://doc.rust-lang.org/rustdoc/index.html">Rustdoc</a>
          documentation for your crates. After annotating your project with
          <a href="https://doc.rust-lang.org/rustdoc/index.html">Rustdoc</a>, the docs
          need to be zipped and uploaded to <i>Kellnr</i>.
        </p>

        <CodeBlock :code="manualUpload" />

        <p>
          To upload the documentation, <i>Kellnr</i> checks that a <i>crate</i> with a
          corresponding version exists and that the user has the right to upload the
          documentation. If no <i>crate</i> with the correct version exists, the
          upload will fail. The authorization mechanism for uploading documentation is
          the same as for <i>crates</i>. The user needs an authentication
          token that is associated with an owner of the package or with an admin user
          to be able to upload the documentation.
        </p>

        <CodeBlock :code="exampleUpload" />
      </v-card-text>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { CodeBlock, SectionHeader, SubsectionHeader } from "../components/common";

const manualUpload = `# Generate documentation for the project
cargo doc

# Package documentation for the upload
cd ./target
zip -r doc.zip ./doc

# Upload documentation to Kellnr --> Replace values in brackets
curl -H "Authorization: {authorization token}" \\
    http{s}://{Kellnr host}/api/v1/docs/{crate name}/{crate version} \\
    --upload-file {docs archive}`;

const exampleUpload = `# Example: Upload documentation to Kellnr
curl -H "Authorization: Xjd83hDh45FLJ58bdShd4uhVNdnded4f" \\
    http://kellnr.example.com:8000/api/v1/docs/mycrate/1.0.0 \\
    --upload-file doc.zip`;
</script>

<style scoped>
.docs-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 12px;
  overflow: hidden;
}

.docs-content {
  padding: 24px;
  color: rgb(var(--v-theme-on-surface));
  font-size: 1rem;
  line-height: 1.6;
}

.docs-content p {
  margin-bottom: 16px;
}

a {
  color: rgb(var(--v-theme-primary));
  text-decoration: underline;
}
</style>
