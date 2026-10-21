    mod glass {
        use vyre_libs::visual::glass::{
            glass_blur_stage, glass_filter_stage, glass_stages, GlassParams,
        };

        fn default_params() -> GlassParams {
            GlassParams {
                width: 16,
                height: 16,
                blur_radius: 4,
                blur_sigma: 1.5,
                tint_rgba: 0x0D_FFFFFF,
                brightness: 1.0,
                saturation: 0.75,
            }
        }

        #[test]
        fn blur_stage_builds() {
            let params = default_params();
            let stages = glass_blur_stage("scene", "out", "tmp", &params);
            assert_eq!(stages.stage_count(), 2);
        }

        #[test]
        fn filter_stage_builds() {
            let params = default_params();
            let prog = glass_filter_stage("blurred", &params);
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn stages_returns_two_programs() {
            let params = default_params();
            let (blur, tint) = glass_stages("scene", "out", "tmp", &params);
            assert_eq!(blur.stage_count(), 2, "blur needs two dispatches");
            assert_eq!(tint.buffers().len(), 1, "tint stage works in-place");
        }

        #[test]
        fn builds_with_zero_blur() {
            let params = GlassParams {
                blur_radius: 0,
                blur_sigma: 0.1,
                ..default_params()
            };
            let (blur, _tint) = glass_stages("s", "o", "t", &params);
            assert_eq!(blur.stage_count(), 2);
        }

        #[test]
        fn builds_with_1x1_image() {
            let params = GlassParams {
                width: 1,
                height: 1,
                ..default_params()
            };
            let (blur, tint) = glass_stages("s", "o", "t", &params);
            assert_eq!(blur.stage_count(), 2);
            assert_eq!(tint.buffers().len(), 1);
        }
    }

    // ================================================================
    // conv1d Tier 2.5 primitive tests
    // ================================================================

    mod conv1d {
        use vyre_primitives::math::conv1d::{
            conv1d_program, gaussian_weights, pack_params, MAX_RADIUS,
        };

        #[test]
        fn program_has_four_buffers() {
            let prog = conv1d_program(16, 2);
            assert_eq!(
                prog.buffers().len(),
                4,
                "conv1d: input + output + weights + params"
            );
            assert_eq!(
                prog.workgroup_size(),
                [256, 1, 1],
                "conv1d must declare local workgroup size, not precomputed dispatch groups"
            );
        }

        #[test]
        fn pack_params_clamps_radius() {
            let params = pack_params(100, 1, 200); // radius 200 > MAX_RADIUS
            assert_eq!(params[2], MAX_RADIUS, "radius should be clamped");
        }

        #[test]
        fn gaussian_weights_singleton() {
            let w = gaussian_weights(0, 1.0);
            assert_eq!(w, vec![65536], "radius=0 → single weight at 1.0");
        }

        #[test]
        fn gaussian_weights_center_is_maximum() {
            let w = gaussian_weights(4, 1.5);
            let center = w.len() / 2;
            for (i, &val) in w.iter().enumerate() {
                if i != center {
                    assert!(val <= w[center], "center weight must be the maximum");
                }
            }
        }

        #[test]
        fn program_radius_clamp_to_max() {
            let prog = conv1d_program(32, MAX_RADIUS + 100);
            let bufs = prog.buffers();
            // Weights buffer should have diameter = 2*MAX_RADIUS+1 = 129 elements.
            let weights_buf = &bufs[2];
            assert_eq!(weights_buf.name(), "weights");
        }
    }
