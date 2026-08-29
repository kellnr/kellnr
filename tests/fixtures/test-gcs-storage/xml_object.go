// Package-local additions for kellnr's GCS integration tests.
//
// Upstream fake-gcs-server does not implement the GCS XML API object routes
// (https://github.com/fsouza/fake-gcs-server/issues/1164), but `object_store`, the GCS
// client kellnr uses, talks the XML API exclusively. Without these handlers a
// `PUT /{bucket}/{object}` falls through to the JSON "signed URL" route and fails with
// "invalid uploadType".
//
// The handlers below are ported from the `support-xml-api` branch of
// https://github.com/tustvold/fake-gcs-server, the same patch `object_store`'s own CI
// runs against. This file is copied into the upstream source tree at image build time,
// see the Dockerfile next to it.

package fakestorage

import (
	"net/http"
	"net/url"
	"strings"

	"github.com/fsouza/fake-gcs-server/internal/backend"
	"github.com/gorilla/mux"
)

// xmlPutObject implements https://cloud.google.com/storage/docs/xml-api/put-object-upload
func (s *Server) xmlPutObject(r *http.Request) xmlResponse {
	vars := unescapeMuxVars(mux.Vars(r))
	defer r.Body.Close()

	if _, err := s.backend.GetBucket(vars["bucketName"]); err != nil {
		return xmlResponse{status: http.StatusNotFound}
	}

	metaData := make(map[string]string)
	for key := range r.Header {
		lowerKey := strings.ToLower(key)
		if metaDataKey := strings.TrimPrefix(lowerKey, "x-goog-meta-"); metaDataKey != lowerKey {
			metaData[metaDataKey] = r.Header.Get(key)
		}
	}

	obj := StreamingObject{
		ObjectAttrs: ObjectAttrs{
			BucketName:      vars["bucketName"],
			Name:            vars["objectName"],
			ContentType:     r.Header.Get(contentTypeHeader),
			ContentEncoding: r.Header.Get(contentEncodingHeader),
			Metadata:        metaData,
		},
	}

	if source := r.Header.Get("x-goog-copy-source"); source != "" {
		escaped, err := url.PathUnescape(source)
		if err != nil {
			return xmlResponse{status: http.StatusBadRequest}
		}

		split := strings.SplitN(escaped, "/", 2)
		if len(split) != 2 {
			return xmlResponse{status: http.StatusBadRequest}
		}

		sourceObject, err := s.GetObjectStreaming(split[0], split[1])
		if err != nil {
			return xmlResponse{status: http.StatusNotFound}
		}
		obj.Content = sourceObject.Content
	} else {
		obj.Content = notImplementedSeeker{r.Body}
	}

	obj, err := s.createObject(obj, backend.NoConditions{})
	if err != nil {
		return xmlResponse{
			status:       http.StatusInternalServerError,
			errorMessage: err.Error(),
		}
	}
	obj.Close()

	header := make(http.Header)
	header.Set("ETag", obj.Etag)

	return xmlResponse{
		status: http.StatusOK,
		header: header,
	}
}

// xmlDeleteObject implements https://cloud.google.com/storage/docs/xml-api/delete-object
// by reusing the JSON API handler, which takes the same mux variables.
func (s *Server) xmlDeleteObject(r *http.Request) xmlResponse {
	resp := s.deleteObject(r)
	return xmlResponse{
		status:       resp.status,
		errorMessage: resp.errorMessage,
		header:       resp.header,
	}
}
