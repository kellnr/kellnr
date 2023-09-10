<template>
  <div class="glass">
    <h1 class="k-h1">Publish Crate Documentation</h1>

    <p class="k-p">
      If enabled, <a href="https://doc.rust-lang.org/rustdoc/index.html" class="link">Rustdoc</a> documentation gets auto-generated for each uploaded <i>crate</i>.
      If you just uploaded a new <i>crate</i> and no documentation is available, the <i>crate</i> is still in the queue and will be processed soon. For more information about the documentation features of <i>Kellnr</i> see the official <a href="https://kellnr.io/documentation" class="link">documentation</a>.
      If for some reason, the documentation generation fails or a custom documentation should be used, see below for instruction for manual documentation uploads.
    </p>

    <h2 class="k-h2">Manual publishing Documentation</h2>
    <p class="k-p">
      <i>Kellnr</i> provides an easy option to host the corresponding
      <a href="https://doc.rust-lang.org/rustdoc/index.html" class="link">Rustdoc</a>
      documentation for your crates. After annotating your project with
      <a href="https://doc.rust-lang.org/rustdoc/index.html" class="link">Rustdoc</a>, the docs
      need to be zipped and uploaded to <i>Kellnr</i>.
    </p>

    <pre v-highlightjs class="glass"><code class="bash"># Generate documentation for the project
cargo doc

# Package documentation for the upload
cd ./target
zip -r doc.zip ./doc

# Upload documentation to Kellnr --> Replace values in brackets
curl -H "Authorization: {authorization token}" \
    http{s}://{Kellnr host}/api/v1/docs/{crate name}/{crate version} \
    --upload-file {docs archive}</code></pre>

    <p class="k-p">
      To upload the documentation, <i>Kellnr</i> checks that a <i>crate</i> with a
      corresponding version exists and that the user has the right to upload the
      documentation. If no <i>crate</i> with the correct version exists, the
      upload will fail. The authorization mechanism for uploading documentation is
      the same like it is used for <i>crates</i>. The user needs an authentication
      token that is associated with an owner of the package or with an admin user
      to be able to upload the documentation.
    </p>

    <pre v-highlightjs class="glass"><code class="bash"># Example: Upload documentation to Kellnr
curl -H "Authorization: Xjd83hDh45FLJ58bdShd4uhVNdnded4f" \
    http://kellnr.example.com:8000/api/v1/docs/mycrate/1.0.0 \
    --upload-file doc.zip</code></pre>
  </div>
</template>

<script setup lang="ts">
</script>

<style scoped>

code {
    background-color: transparent;
    font-size: 1rem;
    padding: 0;
    margin: 0;;
}

/*Dark theme*/

body[color-theme="dark"] code {
    color: var(--dark-color-white);
}

a {
  /*under line text*/
  text-decoration: underline;
}
</style>
